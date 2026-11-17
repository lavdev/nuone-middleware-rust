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
        init_erp::{InitCommand, InitErpUseCase, InitError},
        menu_navigation::{MenuCommand, MenuError, MenuNavigationUseCase},
        session_lifecycle::{
            DisconnectSessionUseCase, ForceCloseSessionUseCase, SessionLifecycleError,
        },
    },
};

#[derive(Clone)]
pub struct HttpState {
    crypto: Arc<dyn CryptoPort>,
    authenticate: Arc<dyn AuthenticateUseCase>,
    init: Arc<dyn InitErpUseCase>,
    menu: Arc<dyn MenuNavigationUseCase>,
    disconnect: Arc<dyn DisconnectSessionUseCase>,
    force_close: Arc<dyn ForceCloseSessionUseCase>,
    agent_status: Arc<dyn AgentStatusUseCase>,
    health: HealthCheckService,
    development_mode: bool,
}

impl HttpState {
    pub fn new(
        crypto: Arc<dyn CryptoPort>,
        authenticate: Arc<dyn AuthenticateUseCase>,
        init: Arc<dyn InitErpUseCase>,
        menu: Arc<dyn MenuNavigationUseCase>,
        disconnect: Arc<dyn DisconnectSessionUseCase>,
        force_close: Arc<dyn ForceCloseSessionUseCase>,
        agent_status: Arc<dyn AgentStatusUseCase>,
        health: HealthCheckService,
        development_mode: bool,
    ) -> Self {
        Self {
            crypto,
            authenticate,
            init,
            menu,
            disconnect,
            force_close,
            agent_status,
            health,
            development_mode,
        }
    }
}

pub fn router(state: HttpState) -> Router {
    Router::new()
        .route("/connect", post(connect))
        .route("/init", post(init))
        .route("/menu", post(menu))
        .route("/disconnect", post(disconnect))
        .route("/close/{session_id}", post(force_close))
        .route("/agent/status", get(agent_status))
        .route("/actuator/health", get(actuator_health))
        .route("/", get(health))
        .with_state(state)
}

async fn init(State(state): State<HttpState>, headers: HeaderMap, body: String) -> Response {
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
    let request: Value = match serde_json::from_str(&plaintext) {
        Ok(request) => request,
        Err(_) => {
            return HttpError::new(
                StatusCode::BAD_REQUEST,
                "400",
                "Invalid initialization request",
            )
            .into_response();
        }
    };
    let session_id = session_id_alias(&request).unwrap_or_default().to_owned();
    log_development_payload(
        &state,
        "init",
        &session_id,
        "gateway-to-middleware",
        &request,
    );
    match state
        .init
        .initialize(InitCommand {
            session_id: session_id.clone(),
        })
        .await
    {
        Ok(result) => {
            let encrypted = match state.crypto.encrypt_http_body(&result.content_json) {
                Ok(encrypted) => encrypted,
                Err(error) => return crypto_error(error).into_response(),
            };
            let response = serde_json::json!({ "content": { "pack": encrypted } });
            log_development_payload(
                &state,
                "init",
                &session_id,
                "middleware-to-gateway",
                &payload_from_text(&result.content_json),
            );
            Json(response).into_response()
        }
        Err(error) => {
            let error = init_error(error);
            log_development_payload(
                &state,
                "init",
                &session_id,
                "middleware-to-gateway",
                &error.payload(),
            );
            error.into_response()
        }
    }
}

async fn menu(State(state): State<HttpState>, headers: HeaderMap, body: String) -> Response {
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
    let request: Value = match serde_json::from_str(&plaintext) {
        Ok(request) => request,
        Err(_) => {
            return HttpError::new(StatusCode::BAD_REQUEST, "400", "Invalid menu request")
                .into_response();
        }
    };
    let session_id = session_id_alias(&request).unwrap_or_default().to_owned();
    let command = MenuCommand {
        session_id: session_id.clone(),
        click: request
            .get("click")
            .and_then(Value::as_str)
            .map(str::to_owned),
        menu_type: request
            .get("type")
            .and_then(Value::as_str)
            .map(str::to_owned),
    };
    log_development_payload(
        &state,
        "menu",
        &session_id,
        "gateway-to-middleware",
        &request,
    );
    match state.menu.navigate(command).await {
        Ok(result) => match serde_json::from_str::<Value>(&result.content_json) {
            Ok(response) => {
                log_development_payload(
                    &state,
                    "menu",
                    &session_id,
                    "middleware-to-gateway",
                    &response,
                );
                Json(response).into_response()
            }
            Err(_) => HttpError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "500",
                "Failed to serialize menu response",
            )
            .into_response(),
        },
        Err(error) => {
            let error = menu_error(error);
            log_development_payload(
                &state,
                "menu",
                &session_id,
                "middleware-to-gateway",
                &error.payload(),
            );
            error.into_response()
        }
    }
}

fn session_id_alias(request: &Value) -> Option<&str> {
    request
        .get("sessionID")
        .and_then(Value::as_str)
        .or_else(|| request.get("sessionId").and_then(Value::as_str))
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
    log_development_payload(
        &state,
        "connect",
        "-",
        "gateway-to-middleware",
        &payload_from_text(&plaintext),
    );
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
            Ok(json) => {
                log_development_payload(
                    &state,
                    "connect",
                    &result.session_id,
                    "middleware-to-gateway",
                    &payload_from_text(&json),
                );
                plain_text(json)
            }
            Err(_) => HttpError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "500",
                "Failed to serialize authentication response",
            )
            .into_response(),
        },
        Err(error) => {
            let error = auth_error(error);
            log_development_payload(
                &state,
                "connect",
                "-",
                "middleware-to-gateway",
                &error.payload(),
            );
            error.into_response()
        }
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
    log_development_payload(&state, "disconnect", "-", "gateway-to-middleware", &value);
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
        Ok(_) => {
            log_development_payload(
                &state,
                "disconnect",
                session_id,
                "middleware-to-gateway",
                &Value::String("logout".to_owned()),
            );
            plain_text("logout".to_owned())
        }
        Err(error) => {
            let error = lifecycle_error(error);
            log_development_payload(
                &state,
                "disconnect",
                session_id,
                "middleware-to-gateway",
                &error.payload(),
            );
            error.into_response()
        }
    }
}

async fn force_close(State(state): State<HttpState>, Path(session_id): Path<String>) -> Response {
    log_development_payload(
        &state,
        "close",
        &session_id,
        "gateway-to-middleware",
        &serde_json::json!({ "sessionId": session_id }),
    );
    match state.force_close.force_close(&session_id).await {
        Ok(result) => {
            if let Ok(payload) = serde_json::to_value(&result) {
                log_development_payload(
                    &state,
                    "close",
                    &session_id,
                    "middleware-to-gateway",
                    &payload,
                );
            }
            Json(result).into_response()
        }
        Err(error) => {
            let error = lifecycle_error(error);
            log_development_payload(
                &state,
                "close",
                &session_id,
                "middleware-to-gateway",
                &error.payload(),
            );
            error.into_response()
        }
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
    log_development_payload(
        &state,
        "agent/status",
        &query.session_id,
        "gateway-to-middleware",
        &serde_json::json!({ "sessionId": query.session_id }),
    );
    match state.agent_status.status(&query.session_id) {
        Ok(result) => {
            if let Ok(payload) = serde_json::to_value(&result) {
                log_development_payload(
                    &state,
                    "agent/status",
                    &query.session_id,
                    "middleware-to-gateway",
                    &payload,
                );
            }
            Json(result).into_response()
        }
        Err(AgentStatusError::InvalidRequest) => {
            let error = HttpError::new(
                StatusCode::BAD_REQUEST,
                "400",
                "Invalid agent status request",
            );
            log_development_payload(
                &state,
                "agent/status",
                &query.session_id,
                "middleware-to-gateway",
                &error.payload(),
            );
            error.into_response()
        }
        Err(AgentStatusError::InvalidSession) => {
            let error = HttpError::new(StatusCode::BAD_REQUEST, "400", "Invalid session");
            log_development_payload(
                &state,
                "agent/status",
                &query.session_id,
                "middleware-to-gateway",
                &error.payload(),
            );
            error.into_response()
        }
        Err(AgentStatusError::Repository) => {
            let error = HttpError::internal();
            log_development_payload(
                &state,
                "agent/status",
                &query.session_id,
                "middleware-to-gateway",
                &error.payload(),
            );
            error.into_response()
        }
    }
}

async fn health(State(state): State<HttpState>) -> Json<HealthStatus> {
    let result = state.health.check();
    if let Ok(payload) = serde_json::to_value(&result) {
        log_development_payload(&state, "health", "-", "middleware-to-gateway", &payload);
    }
    Json(result)
}

async fn actuator_health(State(state): State<HttpState>) -> Json<ActuatorHealthStatus> {
    let result = state.health.actuator_check();
    if let Ok(payload) = serde_json::to_value(&result) {
        log_development_payload(
            &state,
            "actuator/health",
            "-",
            "middleware-to-gateway",
            &payload,
        );
    }
    Json(result)
}

fn log_development_payload(
    state: &HttpState,
    operation: &str,
    session_id: &str,
    direction: &str,
    payload: &Value,
) {
    if !state.development_mode {
        return;
    }
    let sanitized = sanitize_payload(payload);
    tracing::info!(
        target: "nuone::http_payload",
        operation,
        session_id,
        direction,
        payload = %sanitized,
        "HTTP endpoint payload"
    );
}

fn payload_from_text(text: &str) -> Value {
    serde_json::from_str(text)
        .unwrap_or_else(|_| Value::String("[unparseable payload omitted]".to_owned()))
}

fn sanitize_payload(value: &Value) -> Value {
    const SENSITIVE_KEY_PARTS: [&str; 8] = [
        "password",
        "passwd",
        "cipher",
        "token",
        "secret",
        "credential",
        "authorization",
        "privatekey",
    ];

    match value {
        Value::Object(fields) => {
            let mut sanitized = serde_json::Map::new();
            for (key, value) in fields {
                let key_lower = key.to_ascii_lowercase();
                let is_sensitive = SENSITIVE_KEY_PARTS
                    .iter()
                    .any(|part| key_lower.contains(part));
                sanitized.insert(
                    key.clone(),
                    if is_sensitive {
                        Value::String("[REDACTED]".to_owned())
                    } else {
                        sanitize_payload(value)
                    },
                );
            }
            Value::Object(sanitized)
        }
        Value::Array(items) => Value::Array(items.iter().map(sanitize_payload).collect()),
        _ => value.clone(),
    }
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

fn init_error(error: InitError) -> HttpError {
    match error {
        InitError::InvalidRequest => HttpError::new(
            StatusCode::BAD_REQUEST,
            "400",
            "Invalid initialization request",
        ),
        InitError::InvalidSession => {
            HttpError::new(StatusCode::BAD_REQUEST, "400", "Invalid session")
        }
        InitError::NotAuthenticated => HttpError::new(
            StatusCode::BAD_REQUEST,
            "400",
            "Session is not authenticated",
        ),
        InitError::Timeout => HttpError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "500",
            "ERP initialization timed out",
        ),
        InitError::Gateway(_)
        | InitError::Repository(_)
        | InitError::SessionLock(_)
        | InitError::Translation(_) => HttpError::internal(),
    }
}

fn menu_error(error: MenuError) -> HttpError {
    match error {
        MenuError::InvalidRequest => {
            HttpError::new(StatusCode::BAD_REQUEST, "400", "Invalid menu request")
        }
        MenuError::InvalidSession => {
            HttpError::new(StatusCode::BAD_REQUEST, "400", "Invalid session")
        }
        MenuError::NotInitialized => {
            HttpError::new(StatusCode::BAD_REQUEST, "400", "Session is not initialized")
        }
        MenuError::MissingApplication => HttpError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "500",
            "Session application is missing",
        ),
        MenuError::MissingMenuId => {
            HttpError::new(StatusCode::BAD_REQUEST, "400", "Menu item id is required")
        }
        MenuError::Timeout => HttpError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "500",
            "ERP menu polling timed out",
        ),
        MenuError::InvalidMenuResponse => HttpError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "500",
            "Invalid ERP menu response",
        ),
        MenuError::UnknownMenuType => HttpError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "500",
            "Unknown ERP menu type",
        ),
        MenuError::Gateway(_)
        | MenuError::Repository(_)
        | MenuError::SessionLock(_)
        | MenuError::Translation(_) => HttpError::internal(),
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

    fn payload(&self) -> Value {
        serde_json::json!({
            "error": true,
            "code": self.code,
            "message": self.message,
        })
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
