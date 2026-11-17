//! Menu listing and menu item execution against an initialized ERP session.

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
        CLIENT_ACK_OK, MENU_TYPE_CLASS, MENU_TYPE_METHOD, MENU_TYPE_MODULE, PROTO_EVENT_CLICK,
        PROTO_FINALIZER, PROTO_MODULES, PROTO_SEPARATOR,
    },
};

const METHOD_TYPE: &str = "MENU_TYPE_METHOD";

#[derive(Clone, Debug)]
pub struct MenuCommand {
    pub session_id: String,
    pub click: Option<String>,
    pub menu_type: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MenuResult {
    pub content_json: String,
}

#[derive(Debug, thiserror::Error)]
pub enum MenuError {
    #[error("invalid menu request")]
    InvalidRequest,
    #[error("session was not found")]
    InvalidSession,
    #[error("session is not initialized")]
    NotInitialized,
    #[error("session application is missing")]
    MissingApplication,
    #[error("menu item id is required")]
    MissingMenuId,
    #[error("ERP menu polling timed out")]
    Timeout,
    #[error("invalid ERP menu response")]
    InvalidMenuResponse,
    #[error("unknown ERP menu type")]
    UnknownMenuType,
    #[error("ERP menu operation failed")]
    Gateway(#[from] GatewayError),
    #[error("session state could not be read")]
    Repository(#[from] SessionRepositoryError),
    #[error("session operation could not be serialized")]
    SessionLock(#[from] SessionOperationLockError),
    #[error("ERP response could not be translated")]
    Translation(#[from] ProtoToJsonError),
}

#[async_trait]
pub trait MenuNavigationUseCase: Send + Sync {
    async fn navigate(&self, command: MenuCommand) -> Result<MenuResult, MenuError>;
}

pub struct MenuNavigationService<G, S, L, P> {
    gateway: G,
    sessions: S,
    locks: L,
    translator: P,
    timeout: Duration,
}

impl<G, S, L, P> MenuNavigationService<G, S, L, P> {
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
impl<G, S, L, P> MenuNavigationUseCase for MenuNavigationService<G, S, L, P>
where
    G: BotGateway,
    S: SessionRepository,
    L: SessionOperationLock,
    P: ProtoToJsonPort,
{
    async fn navigate(&self, command: MenuCommand) -> Result<MenuResult, MenuError> {
        if command.session_id.trim().is_empty() {
            return Err(MenuError::InvalidRequest);
        }
        let session = self
            .sessions
            .find_by_id(&command.session_id)?
            .ok_or(MenuError::InvalidSession)?;
        if !session.authenticated() || !session.initialized() {
            return Err(MenuError::NotInitialized);
        }
        let _session_guard = self.locks.acquire(&command.session_id).await?;

        if command.menu_type.as_deref() == Some(METHOD_TYPE) {
            return self.execute_item(&command).await;
        }
        match command
            .click
            .as_deref()
            .filter(|click| !click.trim().is_empty())
        {
            None => {
                self.list_root(&command.session_id, session.application())
                    .await
            }
            Some(parent) => self.list_submodules(&command.session_id, parent).await,
        }
    }
}

impl<G, S, L, P> MenuNavigationService<G, S, L, P>
where
    G: BotGateway,
    S: SessionRepository,
    L: SessionOperationLock,
    P: ProtoToJsonPort,
{
    async fn list_root(
        &self,
        session_id: &str,
        application: Option<&str>,
    ) -> Result<MenuResult, MenuError> {
        let application = application
            .filter(|value| !value.trim().is_empty())
            .ok_or(MenuError::MissingApplication)?;
        self.gateway
            .send_to_session(
                session_id,
                &format!("{PROTO_MODULES}{PROTO_SEPARATOR}{application}"),
            )
            .await?;
        let response = self.receive_non_ack(session_id).await?;
        Ok(MenuResult {
            content_json: parse_modules(&response)?,
        })
    }

    async fn list_submodules(
        &self,
        session_id: &str,
        parent: &str,
    ) -> Result<MenuResult, MenuError> {
        self.send_click(session_id, parent).await?;
        let response = self.gateway.receive_session(session_id).await?;
        let content_json = if contains_menu_type(&response) {
            parse_modules(&response)?
        } else {
            self.translator.translate(&response)?
        };
        Ok(MenuResult { content_json })
    }

    async fn execute_item(&self, command: &MenuCommand) -> Result<MenuResult, MenuError> {
        let item = command
            .click
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .ok_or(MenuError::MissingMenuId)?;
        self.send_click(&command.session_id, item).await?;
        let response = self.gateway.receive_session(&command.session_id).await?;
        Ok(MenuResult {
            content_json: self.translator.translate(&response)?,
        })
    }

    async fn send_click(&self, session_id: &str, id: &str) -> Result<(), MenuError> {
        self.gateway
            .send_to_session(
                session_id,
                &format!("{PROTO_EVENT_CLICK}{PROTO_SEPARATOR}{id}"),
            )
            .await?;
        Ok(())
    }

    async fn receive_non_ack(&self, session_id: &str) -> Result<String, MenuError> {
        tokio::time::timeout(self.timeout, async {
            loop {
                let response = self.gateway.receive_session(session_id).await?;
                if !response.contains(CLIENT_ACK_OK) {
                    return Ok::<_, GatewayError>(response);
                }
            }
        })
        .await
        .map_err(|_| MenuError::Timeout)?
        .map_err(MenuError::Gateway)
    }
}

fn parse_modules(response: &str) -> Result<String, MenuError> {
    let normalized = response.replace(PROTO_FINALIZER, "");
    let values = normalized.split(PROTO_SEPARATOR).collect::<Vec<_>>();
    if values.len() < 4 || (values.len() - 1) % 3 != 0 {
        return Err(MenuError::InvalidMenuResponse);
    }
    let mut modules = Vec::new();
    for entry in values[1..].chunks_exact(3) {
        let menu_type = match entry[2] {
            MENU_TYPE_MODULE => "MENU_TYPE_MODULE",
            MENU_TYPE_CLASS => "MENU_TYPE_CLASS",
            MENU_TYPE_METHOD => "MENU_TYPE_METHOD",
            _ => return Err(MenuError::UnknownMenuType),
        };
        modules.push(serde_json::json!({ "menu": entry[0], "id": entry[1], "type": menu_type }));
    }
    serde_json::to_string(&serde_json::json!({ "modules": modules }))
        .map_err(|_| MenuError::InvalidMenuResponse)
}

fn contains_menu_type(response: &str) -> bool {
    response.contains(MENU_TYPE_MODULE)
        || response.contains(MENU_TYPE_CLASS)
        || response.contains(MENU_TYPE_METHOD)
}
