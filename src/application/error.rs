#[derive(Debug, thiserror::Error)]
pub enum ApplicationError {
    #[error("invalid request")]
    InvalidRequest,
    #[error("invalid session")]
    InvalidSession,
    #[error("resource not found")]
    NotFound,
    #[error("dependency unavailable")]
    DependencyUnavailable,
    #[error("internal application error")]
    Internal,
}

pub type ApplicationResult<T> = Result<T, ApplicationError>;
