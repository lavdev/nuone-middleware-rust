//! Orchestrates the legacy ERP authentication handshake.

use async_trait::async_trait;

use crate::{
    application::ports::session::{
        BotGateway, GatewayConnectionId, GatewayError, KeepAliveError, SessionKeepAlivePort,
        SessionRepository, SessionRepositoryError,
    },
    domain::{model::Session, protocol::*},
};

const DEFAULT_ERP_PORT: u16 = 3350;

#[derive(Clone)]
pub struct AuthenticationCommand {
    pub server: String,
    pub username: String,
    pub password: String,
    pub application: String,
    pub application_access_name: Option<String>,
    pub vm: Option<String>,
    pub url: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthenticationResult {
    pub session_id: String,
    pub username: String,
    pub application: String,
    pub connected: bool,
    pub authorized: bool,
    pub message: String,
}

#[derive(Debug, thiserror::Error)]
pub enum AuthenticationError {
    #[error("invalid authentication request")]
    InvalidRequest,
    #[error("invalid ERP server address")]
    InvalidServer,
    #[error("ERP rejected version handshake")]
    VersionRejected,
    #[error("ERP initialization failed")]
    InitializationFailed,
    #[error("ERP initialization reported no license")]
    InitLicenseRejected,
    #[error("ERP application start reported no license")]
    StartLicenseRejected,
    #[error("ERP returned an invalid session id")]
    InvalidSessionId,
    #[error("ERP credentials were rejected")]
    InvalidCredentials,
    #[error("only one client connection is allowed")]
    DuplicateSession,
    #[error("ERP gateway operation failed")]
    Gateway(#[from] GatewayError),
    #[error("session persistence failed")]
    Repository(#[from] SessionRepositoryError),
    #[error("keep-alive lifecycle operation failed")]
    KeepAlive(#[from] KeepAliveError),
}

#[async_trait]
pub trait AuthenticateUseCase: Send + Sync {
    async fn authenticate(
        &self,
        command: AuthenticationCommand,
    ) -> Result<AuthenticationResult, AuthenticationError>;
}

pub struct AuthenticationService<G, S, K> {
    gateway: G,
    sessions: S,
    keep_alive: K,
}

impl<G, S, K> AuthenticationService<G, S, K> {
    pub fn new(gateway: G, sessions: S, keep_alive: K) -> Self {
        Self {
            gateway,
            sessions,
            keep_alive,
        }
    }
}

#[async_trait]
impl<G, S, K> AuthenticateUseCase for AuthenticationService<G, S, K>
where
    G: BotGateway,
    S: SessionRepository,
    K: SessionKeepAlivePort,
{
    async fn authenticate(
        &self,
        command: AuthenticationCommand,
    ) -> Result<AuthenticationResult, AuthenticationError> {
        validate(&command)?;
        let endpoint = parse_server(&command.server)?;

        if let Some(existing) = self.sessions.find_by_username(&command.username)? {
            let stop_result = self.keep_alive.stop(existing.session_id());
            let disconnect_result = self.gateway.disconnect_session(existing.session_id()).await;
            let remove_result = self.sessions.remove(existing.session_id());
            stop_result?;
            disconnect_result?;
            remove_result?;
        }

        let connection = self.gateway.connect(&endpoint.host, endpoint.port).await?;
        let handshake = self
            .perform_handshake(&connection, &command, &endpoint)
            .await;
        let (session_id, application, access_name) = match handshake {
            Ok(values) => values,
            Err(error) => {
                let _ = self.gateway.disconnect(&connection).await;
                return Err(error);
            }
        };

        let mut session = Session::new(&session_id);
        session.set_username(Some(command.username.clone()));
        session.set_application(Some(application));
        session.set_application_access_name(Some(access_name.clone()));
        session.set_vm(command.vm.clone());
        session.set_server_host(Some(endpoint.host.clone()));
        session.mark_authenticated();

        let saved = match self.sessions.save_if_absent(session.clone()) {
            Ok(saved) => saved,
            Err(error) => {
                let _ = self.gateway.disconnect(&connection).await;
                return Err(error.into());
            }
        };
        if !saved {
            let _ = self.gateway.disconnect(&connection).await;
            return Err(AuthenticationError::DuplicateSession);
        }

        if let Err(error) = self.gateway.bind_session(&connection, &session_id).await {
            let _ = self.sessions.remove(&session_id);
            let _ = self.gateway.disconnect(&connection).await;
            return Err(error.into());
        }

        match self.keep_alive.start(&session_id, &endpoint.host) {
            Ok(active) => {
                session.set_persistent_agent_active(active);
                if let Err(error) = self.sessions.update(session) {
                    let _ = self.keep_alive.stop(&session_id);
                    let _ = self.sessions.remove(&session_id);
                    let _ = self.gateway.disconnect(&connection).await;
                    return Err(error.into());
                }
            }
            Err(error) => {
                let _ = self.sessions.remove(&session_id);
                let _ = self.gateway.disconnect(&connection).await;
                return Err(error.into());
            }
        }

        Ok(AuthenticationResult {
            session_id,
            username: command.username,
            application: access_name,
            connected: true,
            authorized: true,
            message: "User has been authenticated successfully".to_owned(),
        })
    }
}

impl<G, S, K> AuthenticationService<G, S, K>
where
    G: BotGateway,
{
    async fn perform_handshake(
        &self,
        connection: &GatewayConnectionId,
        command: &AuthenticationCommand,
        endpoint: &ServerEndpoint,
    ) -> Result<(String, String, String), AuthenticationError> {
        let version = format!(
            "{PROTO_VERSION}{PROTO_SEPARATOR}0{PROTO_SEPARATOR}{}{PROTO_SEPARATOR}{BOT_VERSION}",
            command.application
        );
        self.gateway.send_handshake(connection, &version).await?;
        if self.gateway.receive(connection).await?.contains(PROTO_ERRO) {
            return Err(AuthenticationError::VersionRejected);
        }

        let init = format!(
            "{PROTO_INIT}{PROTO_SEPARATOR}{}{PROTO_SEPARATOR}{}:{}",
            command.application, endpoint.host, endpoint.port
        );
        self.gateway.send_handshake(connection, &init).await?;
        let init_response = self.gateway.receive(connection).await?;
        if init_response.contains(CLIENT_ACK_NOLICENCE) {
            return Err(AuthenticationError::InitLicenseRejected);
        }
        let init_parts: Vec<&str> = init_response.split(PROTO_SEPARATOR).collect();
        let application = init_parts
            .get(2)
            .copied()
            .unwrap_or(&command.application)
            .to_owned();
        let fallback = command
            .application_access_name
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or(&command.application);
        let access_name = init_parts
            .get(3)
            .map(|value| value.replace(PROTO_FINALIZER, ""))
            .unwrap_or_else(|| fallback.to_owned());

        let start = format!(
            "{PROTO_APP_START}{PROTO_SEPARATOR}{application}{PROTO_SEPARATOR}{}{PROTO_SEPARATOR}2{PROTO_SEPARATOR}J{BOT_VERSION}",
            endpoint.host
        );
        self.gateway.send_handshake(connection, &start).await?;
        let start_response = self.gateway.receive(connection).await?;
        if start_response.contains(CLIENT_ACK_NOLICENCE) || start_response.contains(PROTO_ERRO) {
            return Err(if start_response.contains(CLIENT_ACK_NOLICENCE) {
                AuthenticationError::StartLicenseRejected
            } else {
                AuthenticationError::InitializationFailed
            });
        }
        let start_parts: Vec<&str> = start_response.split(PROTO_SEPARATOR).collect();
        let session_id = start_parts
            .get(1)
            .map(|value| value.replace(PROTO_FINALIZER, ""))
            .filter(|value| !value.trim().is_empty())
            .ok_or(AuthenticationError::InvalidSessionId)?;

        let logon = format!(
            "{PROTO_LOGON}{PROTO_SEPARATOR}{}{PROTO_SEPARATOR}{}{PROTO_SEPARATOR}J{BOT_VERSION}",
            command.username.trim(),
            command.password
        );
        self.gateway
            .send_session_message(connection, &session_id, &logon)
            .await?;
        let logon_response = self.gateway.receive(connection).await?;
        if logon_response.contains(CLIENT_ACK_ASK_LOGON)
            || logon_response.contains("usuario nao validado")
        {
            return Err(AuthenticationError::InvalidCredentials);
        }

        let os_name = std::env::consts::OS;
        let os_message =
            format!("{PROTO_GET_OS_CLIENT}{PROTO_SEPARATOR}{os_name}{PROTO_SEPARATOR}{os_name}");
        self.gateway
            .send_session_message(connection, &session_id, &os_message)
            .await?;
        let _ = self.gateway.receive(connection).await?;
        Ok((session_id, application, access_name))
    }
}

struct ServerEndpoint {
    host: String,
    port: u16,
}

fn validate(command: &AuthenticationCommand) -> Result<(), AuthenticationError> {
    if command.server.trim().is_empty()
        || command.username.trim().is_empty()
        || command.application.trim().is_empty()
    {
        return Err(AuthenticationError::InvalidRequest);
    }
    Ok(())
}

fn parse_server(input: &str) -> Result<ServerEndpoint, AuthenticationError> {
    let value = input.trim();
    let without_scheme = value
        .split_once("://")
        .map(|(_, rest)| rest)
        .unwrap_or(value);
    let authority = without_scheme.split(['/', '?', '#']).next().unwrap_or("");
    if authority.is_empty() {
        return Err(AuthenticationError::InvalidServer);
    }

    let (host, port) = if let Some(rest) = authority.strip_prefix('[') {
        let (host, suffix) = rest
            .split_once(']')
            .ok_or(AuthenticationError::InvalidServer)?;
        if suffix.is_empty() {
            (host, DEFAULT_ERP_PORT)
        } else {
            let port = suffix
                .strip_prefix(':')
                .ok_or(AuthenticationError::InvalidServer)?
                .parse()
                .map_err(|_| AuthenticationError::InvalidServer)?;
            (host, port)
        }
    } else if let Some((host, port)) = authority.rsplit_once(':') {
        if host.contains(':') {
            return Err(AuthenticationError::InvalidServer);
        }
        (
            host,
            port.parse()
                .map_err(|_| AuthenticationError::InvalidServer)?,
        )
    } else {
        (authority, DEFAULT_ERP_PORT)
    };
    if host.trim().is_empty() || port == 0 {
        return Err(AuthenticationError::InvalidServer);
    }
    Ok(ServerEndpoint {
        host: host.to_owned(),
        port,
    })
}

#[cfg(test)]
mod tests {
    use std::{collections::VecDeque, sync::Mutex};

    use crate::{
        adapters::persistence::InMemorySessionRepository,
        application::ports::session::{
            BotGateway, GatewayConnectionId, GatewayError, KeepAliveError, SessionKeepAlivePort,
            SessionKeepAliveStatus,
        },
    };

    use super::*;

    #[derive(Default)]
    struct FakeGateway {
        responses: Mutex<VecDeque<String>>,
        handshakes: Mutex<Vec<String>>,
        session_messages: Mutex<Vec<(String, String)>>,
        disconnected: Mutex<Vec<GatewayConnectionId>>,
    }

    #[async_trait]
    impl BotGateway for FakeGateway {
        async fn connect(
            &self,
            _host: &str,
            _port: u16,
        ) -> Result<GatewayConnectionId, GatewayError> {
            Ok(GatewayConnectionId("connection-1".to_owned()))
        }

        async fn send_handshake(
            &self,
            _connection: &GatewayConnectionId,
            message: &str,
        ) -> Result<(), GatewayError> {
            self.handshakes
                .lock()
                .map_err(|_| GatewayError::Send)?
                .push(message.to_owned());
            Ok(())
        }

        async fn send_session_message(
            &self,
            _connection: &GatewayConnectionId,
            session_id: &str,
            message: &str,
        ) -> Result<(), GatewayError> {
            self.session_messages
                .lock()
                .map_err(|_| GatewayError::Send)?
                .push((session_id.to_owned(), message.to_owned()));
            Ok(())
        }

        async fn send_to_session(
            &self,
            _session_id: &str,
            _message: &str,
        ) -> Result<(), GatewayError> {
            Ok(())
        }

        async fn receive(&self, _connection: &GatewayConnectionId) -> Result<String, GatewayError> {
            self.responses
                .lock()
                .map_err(|_| GatewayError::Receive)?
                .pop_front()
                .ok_or(GatewayError::Receive)
        }

        async fn bind_session(
            &self,
            _connection: &GatewayConnectionId,
            _session_id: &str,
        ) -> Result<(), GatewayError> {
            Ok(())
        }

        async fn disconnect(&self, connection: &GatewayConnectionId) -> Result<(), GatewayError> {
            self.disconnected
                .lock()
                .map_err(|_| GatewayError::Disconnect)?
                .push(connection.clone());
            Ok(())
        }

        async fn disconnect_session(&self, _session_id: &str) -> Result<(), GatewayError> {
            Ok(())
        }
    }

    struct FakeKeepAlive;

    impl SessionKeepAlivePort for FakeKeepAlive {
        fn start(&self, _session_id: &str, _host: &str) -> Result<bool, KeepAliveError> {
            Ok(true)
        }

        fn stop(&self, _session_id: &str) -> Result<(), KeepAliveError> {
            Ok(())
        }

        fn status(&self, _session_id: &str) -> SessionKeepAliveStatus {
            SessionKeepAliveStatus {
                active: true,
                interval_ms: 60_000,
                attempt_count: 0,
                success_count: 0,
                last_attempt_at: None,
                last_success_at: None,
                last_failure_at: None,
                last_error: None,
            }
        }
    }

    fn command(server: &str) -> AuthenticationCommand {
        AuthenticationCommand {
            server: server.to_owned(),
            username: " admin ".to_owned(),
            password: "secret".to_owned(),
            application: "APP".to_owned(),
            application_access_name: Some("MID".to_owned()),
            vm: Some("vm.exe".to_owned()),
            url: None,
        }
    }

    fn successful_responses() -> VecDeque<String> {
        [
            format!("{CLIENT_ACK_OK}{PROTO_FINALIZER}"),
            format!(
                "x{PROTO_SEPARATOR}y{PROTO_SEPARATOR}ERP{PROTO_SEPARATOR}ERP-MID{PROTO_FINALIZER}"
            ),
            format!(
                "x{PROTO_SEPARATOR}erp-session-1{PROTO_SEPARATOR}{CLIENT_ACK_OK}{PROTO_FINALIZER}"
            ),
            format!("{CLIENT_ACK_OK}{PROTO_FINALIZER}"),
            format!("{CLIENT_ACK_OK}{PROTO_FINALIZER}"),
        ]
        .into()
    }

    #[tokio::test]
    async fn authenticates_and_persists_session_after_legacy_handshake() {
        let gateway = FakeGateway {
            responses: Mutex::new(successful_responses()),
            ..FakeGateway::default()
        };
        let repository = InMemorySessionRepository::default();
        let service = AuthenticationService::new(gateway, repository, FakeKeepAlive);

        let result = service
            .authenticate(command("http://erp.local:3351"))
            .await
            .expect("auth succeeds");
        let stored = service
            .sessions
            .find_by_id(&result.session_id)
            .expect("repository available")
            .expect("session saved");

        assert_eq!(result.session_id, "erp-session-1");
        assert_eq!(result.application, "ERP-MID");
        assert!(stored.authenticated());
        assert!(stored.persistent_agent_active());
        assert_eq!(stored.server_host(), Some("erp.local"));
    }

    #[tokio::test]
    async fn disconnects_and_does_not_save_when_erp_rejects_credentials() {
        let mut responses = successful_responses();
        responses.pop_back();
        responses.pop_back();
        responses.push_back(format!("{CLIENT_ACK_ASK_LOGON}{PROTO_FINALIZER}"));
        let gateway = FakeGateway {
            responses: Mutex::new(responses),
            ..FakeGateway::default()
        };
        let repository = InMemorySessionRepository::default();
        let service = AuthenticationService::new(gateway, repository, FakeKeepAlive);

        assert!(matches!(
            service.authenticate(command("erp.local")).await,
            Err(AuthenticationError::InvalidCredentials)
        ));
        assert!(
            service
                .sessions
                .find_by_username(" admin ")
                .expect("repository available")
                .is_none()
        );
        assert_eq!(
            service
                .gateway
                .disconnected
                .lock()
                .expect("mutex available")
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn rejects_invalid_server_before_opening_gateway() {
        let gateway = FakeGateway::default();
        let service = AuthenticationService::new(
            gateway,
            InMemorySessionRepository::default(),
            FakeKeepAlive,
        );

        assert!(matches!(
            service.authenticate(command("")).await,
            Err(AuthenticationError::InvalidRequest)
        ));
        assert!(matches!(
            service.authenticate(command("host:bad-port")).await,
            Err(AuthenticationError::InvalidServer)
        ));
        assert!(
            service
                .gateway
                .disconnected
                .lock()
                .expect("mutex available")
                .is_empty()
        );
    }
}
