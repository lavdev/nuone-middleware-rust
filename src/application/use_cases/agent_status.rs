//! Read-only status of the legacy session keep-alive agent.

use crate::application::ports::session::{SessionKeepAlivePort, SessionRepository};

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentStatusResult {
    pub session_id: String,
    pub active: bool,
    pub interval_ms: u64,
    pub attempt_count: u64,
    pub success_count: u64,
    pub last_attempt_at: Option<String>,
    pub last_success_at: Option<String>,
    pub last_failure_at: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum AgentStatusError {
    #[error("invalid agent status request")]
    InvalidRequest,
    #[error("invalid session")]
    InvalidSession,
    #[error("session repository is unavailable")]
    Repository,
}

pub trait AgentStatusUseCase: Send + Sync {
    fn status(&self, session_id: &str) -> Result<AgentStatusResult, AgentStatusError>;
}

pub struct AgentStatusService<S, K> {
    sessions: S,
    keep_alive: K,
}

impl<S, K> AgentStatusService<S, K> {
    pub fn new(sessions: S, keep_alive: K) -> Self {
        Self {
            sessions,
            keep_alive,
        }
    }
}

impl<S, K> AgentStatusService<S, K>
where
    S: SessionRepository,
    K: SessionKeepAlivePort,
{
    pub fn status(&self, session_id: &str) -> Result<AgentStatusResult, AgentStatusError> {
        if session_id.trim().is_empty() {
            return Err(AgentStatusError::InvalidRequest);
        }
        self.sessions
            .find_by_id(session_id)
            .map_err(|_| AgentStatusError::Repository)?
            .ok_or(AgentStatusError::InvalidSession)?;

        let status = self.keep_alive.status(session_id);
        Ok(AgentStatusResult {
            session_id: session_id.to_owned(),
            active: status.active,
            interval_ms: status.interval_ms,
            attempt_count: status.attempt_count,
            success_count: status.success_count,
            last_attempt_at: format_time(status.last_attempt_at),
            last_success_at: format_time(status.last_success_at),
            last_failure_at: format_time(status.last_failure_at),
            last_error: status.last_error,
        })
    }
}

impl<S, K> AgentStatusUseCase for AgentStatusService<S, K>
where
    S: SessionRepository,
    K: SessionKeepAlivePort,
{
    fn status(&self, session_id: &str) -> Result<AgentStatusResult, AgentStatusError> {
        AgentStatusService::status(self, session_id)
    }
}

fn format_time(value: Option<std::time::SystemTime>) -> Option<String> {
    value.and_then(|instant| {
        chrono::DateTime::<chrono::Utc>::from(instant)
            .to_rfc3339_opts(chrono::SecondsFormat::AutoSi, true)
            .into()
    })
}
