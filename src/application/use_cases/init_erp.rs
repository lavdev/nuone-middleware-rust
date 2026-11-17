//! ERP initialization handshake after authentication.

use std::time::Duration;

use async_trait::async_trait;

use crate::{
    application::ports::{
        proto_to_json::{ProtoToJsonError, ProtoToJsonPort},
        session::{
            BotGateway, GatewayError, SessionOperationLock, SessionOperationLockError,
            SessionRepository, SessionRepositoryError,
        },
    },
    domain::protocol::{
        PROTO_MSGRUN_START, PROTO_P_INIT, PROTO_PREFERENCE_CLIENT_ACTIVATE, PROTO_TOOLBAR_NEW,
        PROTO_W_MESSAGE_DATE,
    },
};

#[derive(Clone, Debug)]
pub struct InitCommand {
    pub session_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InitResult {
    pub session_id: String,
    pub raw_response: String,
    pub content_json: String,
}

#[derive(Debug, thiserror::Error)]
pub enum InitError {
    #[error("invalid initialization request")]
    InvalidRequest,
    #[error("session was not found")]
    InvalidSession,
    #[error("session is not authenticated")]
    NotAuthenticated,
    #[error("ERP initialization timed out")]
    Timeout,
    #[error("ERP initialization operation failed")]
    Gateway(#[from] GatewayError),
    #[error("session state could not be read or saved")]
    Repository(#[from] SessionRepositoryError),
    #[error("session operation could not be serialized")]
    SessionLock(#[from] SessionOperationLockError),
    #[error("ERP response could not be translated")]
    Translation(#[from] ProtoToJsonError),
}

#[async_trait]
pub trait InitErpUseCase: Send + Sync {
    async fn initialize(&self, command: InitCommand) -> Result<InitResult, InitError>;
}

pub struct InitErpService<G, S, L, P> {
    gateway: G,
    sessions: S,
    locks: L,
    translator: P,
    timeout: Duration,
}

impl<G, S, L, P> InitErpService<G, S, L, P> {
    pub fn new(gateway: G, sessions: S, locks: L, translator: P, timeout: Duration) -> Self {
        Self {
            gateway,
            sessions,
            locks,
            translator,
            timeout,
        }
    }
}

#[async_trait]
impl<G, S, L, P> InitErpUseCase for InitErpService<G, S, L, P>
where
    G: BotGateway,
    S: SessionRepository,
    L: SessionOperationLock,
    P: ProtoToJsonPort,
{
    async fn initialize(&self, command: InitCommand) -> Result<InitResult, InitError> {
        if command.session_id.trim().is_empty() {
            return Err(InitError::InvalidRequest);
        }
        let mut session = self
            .sessions
            .find_by_id(&command.session_id)?
            .ok_or(InitError::InvalidSession)?;
        if !session.authenticated() {
            return Err(InitError::NotAuthenticated);
        }
        let _session_guard = self.locks.acquire(&command.session_id).await?;

        self.gateway
            .send_to_session(&command.session_id, PROTO_P_INIT)
            .await?;
        let raw_response = tokio::time::timeout(self.timeout, async {
            let mut response = String::new();
            loop {
                response.push_str(&self.gateway.receive_session(&command.session_id).await?);
                if is_terminal_response(&response) {
                    return Ok::<_, GatewayError>(response);
                }
            }
        })
        .await
        .map_err(|_| InitError::Timeout)??;

        let content_json = self.translator.translate(&raw_response)?;
        session.mark_initialized();
        self.sessions.update(session)?;
        Ok(InitResult {
            session_id: command.session_id,
            raw_response,
            content_json,
        })
    }
}

fn is_terminal_response(response: &str) -> bool {
    response.contains(PROTO_TOOLBAR_NEW)
        || response.contains(PROTO_W_MESSAGE_DATE)
        || response.contains(PROTO_PREFERENCE_CLIENT_ACTIVATE)
        || response.contains(PROTO_MSGRUN_START)
}
