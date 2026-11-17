//! Cleanup flows for interactive ERP sessions.

use async_trait::async_trait;

use crate::application::ports::session::{
    BotGateway, GatewayError, KeepAliveError, SessionKeepAlivePort, SessionRepository,
    SessionRepositoryError,
};

#[derive(Debug, thiserror::Error)]
pub enum SessionLifecycleError {
    #[error("invalid session lifecycle request")]
    InvalidRequest,
    #[error("session was not found")]
    NotFound,
    #[error("session lifecycle operation failed")]
    Gateway(#[from] GatewayError),
    #[error("session lifecycle operation failed")]
    KeepAlive(#[from] KeepAliveError),
    #[error("session lifecycle operation failed")]
    Repository(#[from] SessionRepositoryError),
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionLifecycleResult {
    pub session_id: String,
    pub closed: bool,
    pub message: String,
}

#[async_trait]
pub trait DisconnectSessionUseCase: Send + Sync {
    async fn disconnect(
        &self,
        session_id: &str,
        transaction: bool,
    ) -> Result<SessionLifecycleResult, SessionLifecycleError>;
}

#[async_trait]
pub trait ForceCloseSessionUseCase: Send + Sync {
    async fn force_close(
        &self,
        session_id: &str,
    ) -> Result<SessionLifecycleResult, SessionLifecycleError>;
}

pub struct SessionLifecycleService<G, S, K> {
    gateway: G,
    sessions: S,
    keep_alive: K,
}

impl<G, S, K> SessionLifecycleService<G, S, K> {
    pub fn new(gateway: G, sessions: S, keep_alive: K) -> Self {
        Self {
            gateway,
            sessions,
            keep_alive,
        }
    }

    async fn close_resources(
        &self,
        session_id: &str,
        send_close: bool,
    ) -> Result<(), SessionLifecycleError>
    where
        G: BotGateway,
        S: SessionRepository,
        K: SessionKeepAlivePort,
    {
        self.sessions
            .find_by_id(session_id)?
            .ok_or(SessionLifecycleError::NotFound)?;

        let close_result = if send_close {
            self.gateway
                .send_to_session(session_id, crate::domain::protocol::PROTO_NORMAL_CLOSE)
                .await
        } else {
            Ok(())
        };
        let keep_alive_result = self.keep_alive.stop(session_id);
        let gateway_result = self.gateway.disconnect_session(session_id).await;
        let remove_result = self.sessions.remove(session_id);

        close_result?;
        keep_alive_result?;
        gateway_result?;
        remove_result?;
        Ok(())
    }
}

#[async_trait]
impl<G, S, K> DisconnectSessionUseCase for SessionLifecycleService<G, S, K>
where
    G: BotGateway,
    S: SessionRepository,
    K: SessionKeepAlivePort,
{
    async fn disconnect(
        &self,
        session_id: &str,
        transaction: bool,
    ) -> Result<SessionLifecycleResult, SessionLifecycleError> {
        if session_id.trim().is_empty() {
            return Err(SessionLifecycleError::InvalidRequest);
        }
        self.close_resources(session_id, !transaction).await?;
        Ok(SessionLifecycleResult {
            session_id: session_id.to_owned(),
            closed: true,
            message: "Web Application has been disconnected successfully.".to_owned(),
        })
    }
}

#[async_trait]
impl<G, S, K> ForceCloseSessionUseCase for SessionLifecycleService<G, S, K>
where
    G: BotGateway,
    S: SessionRepository,
    K: SessionKeepAlivePort,
{
    async fn force_close(
        &self,
        session_id: &str,
    ) -> Result<SessionLifecycleResult, SessionLifecycleError> {
        if session_id.trim().is_empty() {
            return Err(SessionLifecycleError::InvalidRequest);
        }
        match self.sessions.find_by_id(session_id)? {
            Some(_) => {
                // Forced close is best-effort, but always removes local state.
                let _ = self.keep_alive.stop(session_id);
                let _ = self.gateway.disconnect_session(session_id).await;
                let _ = self.sessions.remove(session_id)?;
                Ok(SessionLifecycleResult {
                    session_id: session_id.to_owned(),
                    closed: true,
                    message: "Session has been force closed.".to_owned(),
                })
            }
            None => Ok(SessionLifecycleResult {
                session_id: session_id.to_owned(),
                closed: false,
                message: "Session is not active.".to_owned(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{sync::Mutex, time::SystemTime};

    use crate::{
        adapters::persistence::InMemorySessionRepository,
        application::ports::session::{
            GatewayConnectionId, KeepAliveError, SessionKeepAliveStatus,
        },
        domain::model::Session,
    };

    use super::*;

    #[derive(Default)]
    struct FakeGateway {
        messages: Mutex<Vec<String>>,
        disconnected: Mutex<Vec<String>>,
        fail_disconnect: bool,
    }

    #[async_trait]
    impl BotGateway for FakeGateway {
        async fn connect(
            &self,
            _host: &str,
            _port: u16,
        ) -> Result<GatewayConnectionId, GatewayError> {
            Ok(GatewayConnectionId("fake-connection".to_owned()))
        }
        async fn send_handshake(
            &self,
            _connection: &GatewayConnectionId,
            _message: &str,
        ) -> Result<(), GatewayError> {
            Ok(())
        }
        async fn send_session_message(
            &self,
            _connection: &GatewayConnectionId,
            _session_id: &str,
            message: &str,
        ) -> Result<(), GatewayError> {
            self.messages
                .lock()
                .map_err(|_| GatewayError::Send)?
                .push(message.to_owned());
            Ok(())
        }
        async fn send_to_session(
            &self,
            _session_id: &str,
            message: &str,
        ) -> Result<(), GatewayError> {
            self.messages
                .lock()
                .map_err(|_| GatewayError::Send)?
                .push(message.to_owned());
            Ok(())
        }
        async fn receive(&self, _connection: &GatewayConnectionId) -> Result<String, GatewayError> {
            Ok(String::new())
        }
        async fn bind_session(
            &self,
            _connection: &GatewayConnectionId,
            _session_id: &str,
        ) -> Result<(), GatewayError> {
            Ok(())
        }
        async fn disconnect(&self, _connection: &GatewayConnectionId) -> Result<(), GatewayError> {
            Ok(())
        }
        async fn disconnect_session(&self, session_id: &str) -> Result<(), GatewayError> {
            self.disconnected
                .lock()
                .map_err(|_| GatewayError::Disconnect)?
                .push(session_id.to_owned());
            if self.fail_disconnect {
                Err(GatewayError::Disconnect)
            } else {
                Ok(())
            }
        }
    }

    #[derive(Default)]
    struct FakeKeepAlive {
        stopped: Mutex<Vec<String>>,
        fail_stop: bool,
    }

    impl SessionKeepAlivePort for FakeKeepAlive {
        fn start(&self, _session_id: &str, _host: &str) -> Result<bool, KeepAliveError> {
            Ok(true)
        }
        fn stop(&self, session_id: &str) -> Result<(), KeepAliveError> {
            self.stopped
                .lock()
                .map_err(|_| KeepAliveError::Stop)?
                .push(session_id.to_owned());
            if self.fail_stop {
                Err(KeepAliveError::Stop)
            } else {
                Ok(())
            }
        }
        fn status(&self, _session_id: &str) -> SessionKeepAliveStatus {
            SessionKeepAliveStatus {
                active: false,
                interval_ms: 60_000,
                attempt_count: 0,
                success_count: 0,
                last_attempt_at: Option::<SystemTime>::None,
                last_success_at: None,
                last_failure_at: None,
                last_error: None,
            }
        }
    }

    fn session() -> Session {
        let mut session = Session::new("session-1");
        session.set_username(Some("user".to_owned()));
        session.mark_authenticated();
        session
    }

    #[tokio::test]
    async fn normal_disconnect_sends_close_and_removes_session() {
        let gateway = FakeGateway::default();
        let keep_alive = FakeKeepAlive::default();
        let sessions = InMemorySessionRepository::default();
        sessions.save_if_absent(session()).expect("save session");
        let service = SessionLifecycleService::new(gateway, sessions, keep_alive);

        let result = service
            .disconnect("session-1", false)
            .await
            .expect("disconnect succeeds");
        assert!(result.closed);
        assert!(
            service
                .sessions
                .find_by_id("session-1")
                .expect("lookup succeeds")
                .is_none()
        );
        assert_eq!(
            service
                .gateway
                .messages
                .lock()
                .expect("mutex available")
                .as_slice(),
            [crate::domain::protocol::PROTO_NORMAL_CLOSE]
        );
        assert_eq!(
            service
                .gateway
                .disconnected
                .lock()
                .expect("mutex available")
                .as_slice(),
            ["session-1"]
        );
    }

    #[tokio::test]
    async fn transactional_disconnect_skips_protocol_close_but_still_cleans_up() {
        let gateway = FakeGateway::default();
        let sessions = InMemorySessionRepository::default();
        sessions.save_if_absent(session()).expect("save session");
        let service = SessionLifecycleService::new(gateway, sessions, FakeKeepAlive::default());

        service
            .disconnect("session-1", true)
            .await
            .expect("disconnect succeeds");
        assert!(
            service
                .gateway
                .messages
                .lock()
                .expect("mutex available")
                .is_empty()
        );
        assert!(
            service
                .sessions
                .find_by_id("session-1")
                .expect("lookup succeeds")
                .is_none()
        );
    }

    #[tokio::test]
    async fn force_close_removes_local_session_even_if_remote_disconnect_fails() {
        let gateway = FakeGateway {
            fail_disconnect: true,
            ..FakeGateway::default()
        };
        let sessions = InMemorySessionRepository::default();
        sessions.save_if_absent(session()).expect("save session");
        let service = SessionLifecycleService::new(gateway, sessions, FakeKeepAlive::default());

        let result = service
            .force_close("session-1")
            .await
            .expect("force close is best effort");
        assert!(result.closed);
        assert!(
            service
                .sessions
                .find_by_id("session-1")
                .expect("lookup succeeds")
                .is_none()
        );
    }
}
