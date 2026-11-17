use std::error::Error;
use std::sync::Arc;

use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

use crate::{
    adapters::{
        crypto::CryptoAdapter,
        http::{self, HttpState},
        persistence::InMemorySessionRepository,
        protocol::DeflatePackerAdapter,
        tcp::{LegacyPingAgent, TcpBotGateway, TcpCommunication},
    },
    application::{
        ports::{crypto::CryptoPort, packer::PackerPort},
        use_cases::{
            agent_status::{AgentStatusService, AgentStatusUseCase},
            authentication::{AuthenticateUseCase, AuthenticationService},
            health_check::HealthCheckService,
            session_lifecycle::{
                DisconnectSessionUseCase, ForceCloseSessionUseCase, SessionLifecycleService,
            },
        },
    },
    infrastructure::config::Settings,
};

pub async fn run() -> Result<(), Box<dyn Error + Send + Sync>> {
    match dotenvy::dotenv() {
        Ok(_) => {}
        Err(error) if error.not_found() => {}
        Err(error) => return Err(Box::new(error)),
    }

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .try_init()?;

    let settings = Settings::from_env()?;
    let listener = TcpListener::bind(settings.http_bind).await?;
    tracing::info!(address = %settings.http_bind, "Nuone Middleware HTTP server started");

    let crypto = Arc::new(CryptoAdapter::new(&settings.cipher_key));
    let crypto_port: Arc<dyn CryptoPort> = crypto.clone();
    let packer: Arc<dyn PackerPort> = Arc::new(DeflatePackerAdapter);
    let tcp = Arc::new(TcpCommunication::new(
        crypto_port.clone(),
        packer,
        settings.tcp,
    ));
    let gateway = Arc::new(TcpBotGateway::new(tcp));
    let sessions = Arc::new(InMemorySessionRepository::default());
    let keep_alive = Arc::new(LegacyPingAgent::new(settings.legacy_ping));

    let authenticate: Arc<dyn AuthenticateUseCase> = Arc::new(AuthenticationService::new(
        gateway.clone(),
        sessions.clone(),
        keep_alive.clone(),
    ));
    let lifecycle = Arc::new(SessionLifecycleService::new(
        gateway.clone(),
        sessions.clone(),
        keep_alive.clone(),
    ));
    let disconnect: Arc<dyn DisconnectSessionUseCase> = lifecycle.clone();
    let force_close: Arc<dyn ForceCloseSessionUseCase> = lifecycle;
    let agent_status: Arc<dyn AgentStatusUseCase> =
        Arc::new(AgentStatusService::new(sessions, keep_alive.clone()));
    let status = HttpState::new(
        crypto_port,
        authenticate,
        disconnect,
        force_close,
        agent_status,
        HealthCheckService::new(settings.bot_version, settings.mode_suffix),
    );

    axum::serve(listener, http::router(status))
        .with_graceful_shutdown(async {
            if let Err(error) = tokio::signal::ctrl_c().await {
                tracing::warn!(%error, "failed to listen for shutdown signal");
            }
        })
        .await?;
    Ok(())
}
