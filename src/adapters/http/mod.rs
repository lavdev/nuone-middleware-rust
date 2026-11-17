//! HTTP adapters for the externally observable session endpoints.

use std::sync::Arc;

use axum::{
    Json, Router,
    body::Body,
    extract::{Path, Query, State},
    http::{HeaderMap, HeaderValue, StatusCode, header::CONTENT_TYPE},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::Deserialize;
use serde_json::Value;

use crate::application::{
    ports::crypto::{CryptoError, CryptoPort},
    use_cases::{
        agent_status::{AgentStatusError, AgentStatusUseCase},
        authentication::{AuthenticateUseCase, AuthenticationCommand, AuthenticationError},
        health_check::{ActuatorHealthStatus, HealthCheckService, HealthStatus},
        session_lifecycle::{
            DisconnectSessionUseCase, ForceCloseSessionUseCase, SessionLifecycleError,
        },
    },
};

#[derive(Clone)]
pub struct HttpState {
    crypto: Arc<dyn CryptoPort>,
    authenticate: Arc<dyn AuthenticateUseCase>,
    disconnect: Arc<dyn DisconnectSessionUseCase>,
    force_close: Arc<dyn ForceCloseSessionUseCase>,
    agent_status: Arc<dyn AgentStatusUseCase>,
    health: HealthCheckService,
}

impl HttpState {
    pub fn new(
        crypto: Arc<dyn CryptoPort>,
        authenticate: Arc<dyn AuthenticateUseCase>,
        disconnect: Arc<dyn DisconnectSessionUseCase>,
        force_close: Arc<dyn ForceCloseSessionUseCase>,
        agent_status: Arc<dyn AgentStatusUseCase>,
        health: HealthCheckService,
    ) -> Self {
        Self {
            crypto,
            authenticate,
            disconnect,
            force_close,
            agent_status,
            health,
        }
    }
}

pub fn router(state: HttpState) -> Router {
    Router::new()
        .route("/connect", post(connect))
        .route("/disconnect", post(disconnect))
        .route("/close/{session_id}", post(force_close))
        .route("/agent/status", get(agent_status))
        .route("/actuator/health", get(actuator_health))
        .route("/", get(health))
        .with_state(state)
}

async fn connect(State(state): State<HttpState>, headers: HeaderMap, body: String) -> Response {
    if !is_plain_text(&headers) {
        return HttpError::new(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "415",
            "Content-Type must be text/plain",
        )
        .into_response();
    }
    if body.trim().is_empty() {
        return HttpError::new(StatusCode::BAD_REQUEST, "400", "bad request").into_response();
    }
    let plaintext = match state.crypto.decrypt_http_body(&body) {
        Ok(plaintext) => plaintext,
        Err(error) => return crypto_error(error).into_response(),
    };
    let request: AuthenticationRequest = match serde_json::from_str(&plaintext) {
        Ok(request) => request,
        Err(_) => {
            return HttpError::new(
                StatusCode::BAD_REQUEST,
                "400",
                "Invalid authentication request",
            )
            .into_response();
        }
    };
    let command = AuthenticationCommand {
        server: request.server,
        username: request.username,
        password: request.password,
        application: request.application,
        application_access_name: request.application_access_name,
        vm: request.vm,
        url: request.url,
    };
    match state.authenticate.authenticate(command).await {
        Ok(result) => match serde_json::to_string(&result) {
            Ok(json) => plain_text(json),
            Err(_) => HttpError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "500",
                "Failed to serialize authentication response",
            )
            .into_response(),
        },
        Err(error) => auth_error(error).into_response(),
    }
}

async fn disconnect(State(state): State<HttpState>, headers: HeaderMap, body: String) -> Response {
    if !is_plain_text(&headers) {
        return HttpError::new(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "415",
            "Content-Type must be text/plain",
        )
        .into_response();
    }
    if body.trim().is_empty() {
        return HttpError::new(StatusCode::BAD_REQUEST, "400", "bad request").into_response();
    }
    let plaintext = match state.crypto.decrypt_http_body(&body) {
        Ok(plaintext) => plaintext,
        Err(error) => return crypto_error(error).into_response(),
    };
    let value: Value = match serde_json::from_str(&plaintext) {
        Ok(value) => value,
        Err(_) => {
            return HttpError::new(StatusCode::BAD_REQUEST, "400", "Invalid disconnect request")
                .into_response();
        }
    };
    let command = value
        .get("command")
        .filter(|command| command.is_object())
        .unwrap_or(&value);
    let session_id = command
        .get("sessionID")
        .or_else(|| command.get("sessionId"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    let transaction = command
        .get("transaction")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    match state.disconnect.disconnect(session_id, transaction).await {
        Ok(_) => plain_text("logout".to_owned()),
        Err(error) => lifecycle_error(error).into_response(),
    }
}

async fn force_close(State(state): State<HttpState>, Path(session_id): Path<String>) -> Response {
    match state.force_close.force_close(&session_id).await {
        Ok(result) => Json(result).into_response(),
        Err(error) => lifecycle_error(error).into_response(),
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AgentStatusQuery {
    session_id: String,
}

async fn agent_status(
    State(state): State<HttpState>,
    Query(query): Query<AgentStatusQuery>,
) -> Response {
    match state.agent_status.status(&query.session_id) {
        Ok(result) => Json(result).into_response(),
        Err(AgentStatusError::InvalidRequest) => HttpError::new(
            StatusCode::BAD_REQUEST,
            "400",
            "Invalid agent status request",
        )
        .into_response(),
        Err(AgentStatusError::InvalidSession) => {
            HttpError::new(StatusCode::BAD_REQUEST, "400", "Invalid session").into_response()
        }
        Err(AgentStatusError::Repository) => HttpError::internal().into_response(),
    }
}

async fn health(State(state): State<HttpState>) -> Json<HealthStatus> {
    Json(state.health.check())
}

async fn actuator_health(State(state): State<HttpState>) -> Json<ActuatorHealthStatus> {
    Json(state.health.actuator_check())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AuthenticationRequest {
    server: String,
    username: String,
    password: String,
    application: String,
    application_access_name: Option<String>,
    vm: Option<String>,
    url: Option<String>,
}

fn is_plain_text(headers: &HeaderMap) -> bool {
    headers
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .is_some_and(|media_type| media_type.trim().eq_ignore_ascii_case("text/plain"))
}

fn plain_text(body: String) -> Response {
    Response::builder()
        .status(StatusCode::OK)
        .header(
            CONTENT_TYPE,
            HeaderValue::from_static("text/plain; charset=utf-8"),
        )
        .body(Body::from(body))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

fn crypto_error(error: CryptoError) -> HttpError {
    let _ = error;
    HttpError::new(
        StatusCode::BAD_REQUEST,
        "400",
        "Failed to decrypt HTTP body",
    )
}

fn auth_error(error: AuthenticationError) -> HttpError {
    use AuthenticationError as E;
    match error {
        E::InvalidRequest => HttpError::new(
            StatusCode::BAD_REQUEST,
            "400",
            "Invalid authentication request",
        ),
        E::InvalidServer => {
            HttpError::new(StatusCode::BAD_REQUEST, "400", "Invalid ERP server address")
        }
        E::VersionRejected => HttpError::new(StatusCode::INTERNAL_SERVER_ERROR, "500", "VERSION"),
        E::InitLicenseRejected => HttpError::new(StatusCode::BAD_REQUEST, "400", "INIT"),
        E::StartLicenseRejected => HttpError::new(StatusCode::BAD_REQUEST, "400", "LICENSE"),
        E::InitializationFailed => HttpError::new(StatusCode::BAD_REQUEST, "400", "START"),
        E::InvalidSessionId => HttpError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "500",
            "Invalid ERP session response",
        ),
        E::InvalidCredentials => {
            HttpError::new(StatusCode::BAD_REQUEST, "400", "Invalid ERP credentials")
        }
        E::DuplicateSession => HttpError::new(
            StatusCode::BAD_REQUEST,
            "400",
            "Just one client connection is allowed.",
        ),
        E::Gateway(crate::application::ports::session::GatewayError::Connection) => HttpError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "500",
            "Failed to connect to ERP TCP server",
        ),
        E::Gateway(crate::application::ports::session::GatewayError::Send) => HttpError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "500",
            "Failed to send TCP message",
        ),
        E::Gateway(crate::application::ports::session::GatewayError::Receive) => HttpError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "500",
            "Failed to receive TCP message",
        ),
        E::Gateway(crate::application::ports::session::GatewayError::Disconnect) => HttpError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "500",
            "Failed to close TCP socket",
        ),
        E::Gateway(crate::application::ports::session::GatewayError::SessionBound)
        | E::Repository(_)
        | E::KeepAlive(_) => HttpError::internal(),
    }
}

fn lifecycle_error(error: SessionLifecycleError) -> HttpError {
    match error {
        SessionLifecycleError::InvalidRequest => {
            HttpError::new(StatusCode::BAD_REQUEST, "400", "Invalid disconnect request")
        }
        SessionLifecycleError::NotFound => {
            HttpError::new(StatusCode::BAD_REQUEST, "400", "Session not found")
        }
        SessionLifecycleError::Gateway(_)
        | SessionLifecycleError::KeepAlive(_)
        | SessionLifecycleError::Repository(_) => HttpError::internal(),
    }
}

struct HttpError {
    status: StatusCode,
    code: &'static str,
    message: String,
}

impl HttpError {
    fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
        }
    }

    fn internal() -> Self {
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "500",
            "Internal application error",
        )
    }
}

impl IntoResponse for HttpError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(serde_json::json!({
                "error": true,
                "code": self.code,
                "message": self.message,
            })),
        )
            .into_response()
    }
}
