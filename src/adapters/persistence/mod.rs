//! Outbound persistence adapters.

use std::{collections::HashMap, sync::Mutex};

use crate::{
    application::ports::session::{SessionRepository, SessionRepositoryError},
    domain::model::Session,
};

#[derive(Default)]
pub struct InMemorySessionRepository {
    sessions: Mutex<HashMap<String, Session>>,
}

impl SessionRepository for InMemorySessionRepository {
    fn save_if_absent(&self, session: Session) -> Result<bool, SessionRepositoryError> {
        let mut sessions = self
            .sessions
            .lock()
            .map_err(|_| SessionRepositoryError::LockUnavailable)?;
        if sessions.contains_key(session.session_id()) {
            return Ok(false);
        }
        sessions.insert(session.session_id().to_owned(), session);
        Ok(true)
    }

    fn find_by_id(&self, session_id: &str) -> Result<Option<Session>, SessionRepositoryError> {
        self.sessions
            .lock()
            .map_err(|_| SessionRepositoryError::LockUnavailable)
            .map(|sessions| sessions.get(session_id).cloned())
    }

    fn find_by_username(&self, username: &str) -> Result<Option<Session>, SessionRepositoryError> {
        self.sessions
            .lock()
            .map_err(|_| SessionRepositoryError::LockUnavailable)
            .map(|sessions| {
                sessions
                    .values()
                    .find(|session| session.username() == Some(username))
                    .cloned()
            })
    }

    fn update(&self, session: Session) -> Result<(), SessionRepositoryError> {
        let mut sessions = self
            .sessions
            .lock()
            .map_err(|_| SessionRepositoryError::LockUnavailable)?;
        if sessions.contains_key(session.session_id()) {
            sessions.insert(session.session_id().to_owned(), session);
        }
        Ok(())
    }

    fn remove(&self, session_id: &str) -> Result<Option<Session>, SessionRepositoryError> {
        self.sessions
            .lock()
            .map_err(|_| SessionRepositoryError::LockUnavailable)
            .map(|mut sessions| sessions.remove(session_id))
    }
}
