//! Session and ERP gateway ports used by authentication and lifecycle use cases.

use std::{sync::Arc, time::SystemTime};

use async_trait::async_trait;

use crate::domain::model::Session;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct GatewayConnectionId(pub String);

#[derive(Debug, thiserror::Error)]
pub enum GatewayError {
    #[error("ERP connection failed")]
    Connection,
    #[error("ERP send failed")]
    Send,
    #[error("ERP receive failed")]
    Receive,
    #[error("ERP connection close failed")]
    Disconnect,
    #[error("ERP session id is already bound")]
    SessionBound,
}

#[async_trait]
pub trait BotGateway: Send + Sync {
    async fn connect(&self, host: &str, port: u16) -> Result<GatewayConnectionId, GatewayError>;
    async fn send_handshake(
        &self,
        connection: &GatewayConnectionId,
        message: &str,
    ) -> Result<(), GatewayError>;
    async fn send_session_message(
        &self,
        connection: &GatewayConnectionId,
        session_id: &str,
        message: &str,
    ) -> Result<(), GatewayError>;
    async fn send_to_session(&self, session_id: &str, message: &str) -> Result<(), GatewayError>;
    async fn receive(&self, connection: &GatewayConnectionId) -> Result<String, GatewayError>;
    async fn receive_session(&self, _session_id: &str) -> Result<String, GatewayError> {
        Err(GatewayError::Connection)
    }
    async fn bind_session(
        &self,
        connection: &GatewayConnectionId,
        session_id: &str,
    ) -> Result<(), GatewayError>;
    async fn disconnect(&self, connection: &GatewayConnectionId) -> Result<(), GatewayError>;
    async fn disconnect_session(&self, session_id: &str) -> Result<(), GatewayError>;
}

#[derive(Debug, thiserror::Error)]
pub enum SessionRepositoryError {
    #[error("session repository lock is unavailable")]
    LockUnavailable,
}

pub trait SessionRepository: Send + Sync {
    fn save_if_absent(&self, session: Session) -> Result<bool, SessionRepositoryError>;
    fn find_by_id(&self, session_id: &str) -> Result<Option<Session>, SessionRepositoryError>;
    fn find_by_username(&self, username: &str) -> Result<Option<Session>, SessionRepositoryError>;
    fn update(&self, session: Session) -> Result<(), SessionRepositoryError>;
    fn remove(&self, session_id: &str) -> Result<Option<Session>, SessionRepositoryError>;
}

#[derive(Debug, thiserror::Error)]
pub enum SessionOperationLockError {
    #[error("session operation lock is unavailable")]
    Unavailable,
}

pub trait SessionOperationGuard: Send {}

#[async_trait]
pub trait SessionOperationLock: Send + Sync {
    async fn acquire(
        &self,
        session_id: &str,
    ) -> Result<Box<dyn SessionOperationGuard>, SessionOperationLockError>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionKeepAliveStatus {
    pub active: bool,
    pub interval_ms: u64,
    pub attempt_count: u64,
    pub success_count: u64,
    pub last_attempt_at: Option<SystemTime>,
    pub last_success_at: Option<SystemTime>,
    pub last_failure_at: Option<SystemTime>,
    pub last_error: Option<String>,
}

pub trait SessionKeepAlivePort: Send + Sync {
    fn start(&self, session_id: &str, host: &str) -> Result<bool, KeepAliveError>;
    fn stop(&self, session_id: &str) -> Result<(), KeepAliveError>;
    fn status(&self, session_id: &str) -> SessionKeepAliveStatus;
}

#[derive(Debug, thiserror::Error)]
pub enum KeepAliveError {
    #[error("could not start ERP keep-alive")]
    Start,
    #[error("could not stop ERP keep-alive")]
    Stop,
}

#[async_trait]
impl<T: BotGateway + ?Sized> BotGateway for Arc<T> {
    async fn connect(&self, host: &str, port: u16) -> Result<GatewayConnectionId, GatewayError> {
        (**self).connect(host, port).await
    }
    async fn send_handshake(
        &self,
        connection: &GatewayConnectionId,
        message: &str,
    ) -> Result<(), GatewayError> {
        (**self).send_handshake(connection, message).await
    }
    async fn send_session_message(
        &self,
        connection: &GatewayConnectionId,
        session_id: &str,
        message: &str,
    ) -> Result<(), GatewayError> {
        (**self)
            .send_session_message(connection, session_id, message)
            .await
    }
    async fn send_to_session(&self, session_id: &str, message: &str) -> Result<(), GatewayError> {
        (**self).send_to_session(session_id, message).await
    }
    async fn receive(&self, connection: &GatewayConnectionId) -> Result<String, GatewayError> {
        (**self).receive(connection).await
    }
    async fn receive_session(&self, session_id: &str) -> Result<String, GatewayError> {
        (**self).receive_session(session_id).await
    }
    async fn bind_session(
        &self,
        connection: &GatewayConnectionId,
        session_id: &str,
    ) -> Result<(), GatewayError> {
        (**self).bind_session(connection, session_id).await
    }
    async fn disconnect(&self, connection: &GatewayConnectionId) -> Result<(), GatewayError> {
        (**self).disconnect(connection).await
    }
    async fn disconnect_session(&self, session_id: &str) -> Result<(), GatewayError> {
        (**self).disconnect_session(session_id).await
    }
}

impl<T: SessionRepository + ?Sized> SessionRepository for Arc<T> {
    fn save_if_absent(&self, session: Session) -> Result<bool, SessionRepositoryError> {
        (**self).save_if_absent(session)
    }
    fn find_by_id(&self, session_id: &str) -> Result<Option<Session>, SessionRepositoryError> {
        (**self).find_by_id(session_id)
    }
    fn find_by_username(&self, username: &str) -> Result<Option<Session>, SessionRepositoryError> {
        (**self).find_by_username(username)
    }
    fn update(&self, session: Session) -> Result<(), SessionRepositoryError> {
        (**self).update(session)
    }
    fn remove(&self, session_id: &str) -> Result<Option<Session>, SessionRepositoryError> {
        (**self).remove(session_id)
    }
}

#[async_trait]
impl<T: SessionOperationLock + ?Sized> SessionOperationLock for Arc<T> {
    async fn acquire(
        &self,
        session_id: &str,
    ) -> Result<Box<dyn SessionOperationGuard>, SessionOperationLockError> {
        (**self).acquire(session_id).await
    }
}

impl<T: SessionKeepAlivePort + ?Sized> SessionKeepAlivePort for Arc<T> {
    fn start(&self, session_id: &str, host: &str) -> Result<bool, KeepAliveError> {
        (**self).start(session_id, host)
    }
    fn stop(&self, session_id: &str) -> Result<(), KeepAliveError> {
        (**self).stop(session_id)
    }
    fn status(&self, session_id: &str) -> SessionKeepAliveStatus {
        (**self).status(session_id)
    }
}
