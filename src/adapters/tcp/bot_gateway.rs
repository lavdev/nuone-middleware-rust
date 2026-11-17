use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use async_trait::async_trait;
use tokio::{
    net::TcpStream,
    sync::{Mutex, watch},
};

use crate::application::ports::session::{BotGateway, GatewayConnectionId, GatewayError};

use super::TcpCommunication;

pub struct TcpBotGateway {
    communication: Arc<TcpCommunication>,
    connections: Mutex<HashMap<GatewayConnectionId, Arc<ManagedConnection>>>,
    sessions: Mutex<HashMap<String, GatewayConnectionId>>,
    next_id: AtomicU64,
}

struct ManagedConnection {
    stream: Mutex<TcpStream>,
    cancel: watch::Sender<bool>,
}

impl TcpBotGateway {
    pub fn new(communication: Arc<TcpCommunication>) -> Self {
        Self {
            communication,
            connections: Mutex::new(HashMap::new()),
            sessions: Mutex::new(HashMap::new()),
            next_id: AtomicU64::new(1),
        }
    }

    async fn connection(
        &self,
        connection: &GatewayConnectionId,
    ) -> Result<Arc<ManagedConnection>, GatewayError> {
        self.connections
            .lock()
            .await
            .get(connection)
            .cloned()
            .ok_or(GatewayError::Connection)
    }
}

#[async_trait]
impl BotGateway for TcpBotGateway {
    async fn connect(&self, host: &str, port: u16) -> Result<GatewayConnectionId, GatewayError> {
        let stream = self
            .communication
            .connect((host, port))
            .await
            .map_err(|_| GatewayError::Connection)?;
        let id = GatewayConnectionId(self.next_id.fetch_add(1, Ordering::Relaxed).to_string());
        let (cancel, _) = watch::channel(false);
        self.connections.lock().await.insert(
            id.clone(),
            Arc::new(ManagedConnection {
                stream: Mutex::new(stream),
                cancel,
            }),
        );
        Ok(id)
    }

    async fn send_handshake(
        &self,
        connection: &GatewayConnectionId,
        message: &str,
    ) -> Result<(), GatewayError> {
        let connection_state = self.connection(connection).await?;
        let mut cancelled = connection_state.cancel.subscribe();
        let mut stream = tokio::select! {
            _ = cancelled.changed() => return Err(GatewayError::Disconnect),
            stream = connection_state.stream.lock() => stream,
        };
        let wire = self.communication.build_handshake_message(message);
        tokio::select! {
            _ = cancelled.changed() => Err(GatewayError::Disconnect),
            result = self.communication.send(&mut *stream, &wire) =>
                result.map_err(|_| GatewayError::Send),
        }
    }

    async fn send_session_message(
        &self,
        connection: &GatewayConnectionId,
        session_id: &str,
        message: &str,
    ) -> Result<(), GatewayError> {
        let connection_state = self.connection(connection).await?;
        let mut cancelled = connection_state.cancel.subscribe();
        let mut stream = tokio::select! {
            _ = cancelled.changed() => return Err(GatewayError::Disconnect),
            stream = connection_state.stream.lock() => stream,
        };
        let wire = self
            .communication
            .build_session_message(session_id, message);
        tokio::select! {
            _ = cancelled.changed() => Err(GatewayError::Disconnect),
            result = self.communication.send(&mut *stream, &wire) =>
                result.map_err(|_| GatewayError::Send),
        }
    }

    async fn send_to_session(&self, session_id: &str, message: &str) -> Result<(), GatewayError> {
        let connection = self
            .sessions
            .lock()
            .await
            .get(session_id)
            .cloned()
            .ok_or(GatewayError::Connection)?;
        self.send_session_message(&connection, session_id, message)
            .await
    }

    async fn receive(&self, connection: &GatewayConnectionId) -> Result<String, GatewayError> {
        let connection_state = self.connection(connection).await?;
        let mut cancelled = connection_state.cancel.subscribe();
        let mut stream = tokio::select! {
            _ = cancelled.changed() => return Err(GatewayError::Disconnect),
            stream = connection_state.stream.lock() => stream,
        };
        tokio::select! {
            _ = cancelled.changed() => Err(GatewayError::Disconnect),
            result = self.communication.receive(&mut *stream) =>
                result.map_err(|_| GatewayError::Receive),
        }
    }

    async fn receive_session(&self, session_id: &str) -> Result<String, GatewayError> {
        let connection = self
            .sessions
            .lock()
            .await
            .get(session_id)
            .cloned()
            .ok_or(GatewayError::Connection)?;
        self.receive(&connection).await
    }

    async fn bind_session(
        &self,
        connection: &GatewayConnectionId,
        session_id: &str,
    ) -> Result<(), GatewayError> {
        let connections = self.connections.lock().await;
        if !connections.contains_key(connection) {
            return Err(GatewayError::Connection);
        }
        let mut sessions = self.sessions.lock().await;
        if sessions.contains_key(session_id) {
            return Err(GatewayError::SessionBound);
        }
        sessions.insert(session_id.to_owned(), connection.clone());
        Ok(())
    }

    async fn disconnect(&self, connection: &GatewayConnectionId) -> Result<(), GatewayError> {
        let removed = self.connections.lock().await.remove(connection);
        if let Some(connection_state) = &removed {
            let _ = connection_state.cancel.send(true);
        }
        self.sessions
            .lock()
            .await
            .retain(|_, value| value != connection);
        if removed.is_some() {
            Ok(())
        } else {
            Err(GatewayError::Disconnect)
        }
    }

    async fn disconnect_session(&self, session_id: &str) -> Result<(), GatewayError> {
        let connection = self
            .sessions
            .lock()
            .await
            .remove(session_id)
            .ok_or(GatewayError::Disconnect)?;
        self.disconnect(&connection).await
    }
}
