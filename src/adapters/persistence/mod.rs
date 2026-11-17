//! Outbound persistence adapters.

use std::{
    collections::HashMap,
    sync::{Mutex, Weak},
};

use async_trait::async_trait;
use tokio::sync::{Mutex as AsyncMutex, OwnedMutexGuard};

use crate::{
    application::ports::session::{
        SessionOperationGuard, SessionOperationLock, SessionOperationLockError, SessionRepository,
        SessionRepositoryError,
    },
    domain::model::Session,
};

#[derive(Default)]
pub struct InMemorySessionRepository {
    sessions: Mutex<HashMap<String, Session>>,
}

#[derive(Default)]
pub struct InMemorySessionOperationLock {
    locks: Mutex<HashMap<String, Weak<AsyncMutex<()>>>>,
}

struct SessionOperationLockGuard {
    _guard: OwnedMutexGuard<()>,
}

impl SessionOperationGuard for SessionOperationLockGuard {}

#[async_trait]
impl SessionOperationLock for InMemorySessionOperationLock {
    async fn acquire(
        &self,
        session_id: &str,
    ) -> Result<Box<dyn SessionOperationGuard>, SessionOperationLockError> {
        let lock = {
            let mut locks = self
                .locks
                .lock()
                .map_err(|_| SessionOperationLockError::Unavailable)?;
            locks.retain(|_, lock| lock.strong_count() > 0);
            if let Some(lock) = locks.get(session_id).and_then(Weak::upgrade) {
                lock
            } else {
                let lock = std::sync::Arc::new(AsyncMutex::new(()));
                locks.insert(session_id.to_owned(), std::sync::Arc::downgrade(&lock));
                lock
            }
        };
        Ok(Box::new(SessionOperationLockGuard {
            _guard: lock.lock_owned().await,
        }))
    }
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
