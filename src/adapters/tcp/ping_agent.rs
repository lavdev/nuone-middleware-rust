use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, SystemTime},
};

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    sync::watch,
    task::JoinHandle,
    time::{sleep, timeout},
};

use crate::application::ports::session::{
    KeepAliveError, SessionKeepAlivePort, SessionKeepAliveStatus,
};

const RESPONSE_END: &[u8] = b"HTTPEAS/";

#[derive(Clone, Copy, Debug)]
pub struct LegacyPingConfig {
    pub enabled: bool,
    pub port: u16,
    pub interval: Duration,
    pub connect_timeout: Duration,
    pub read_timeout: Duration,
}

impl Default for LegacyPingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            port: 4081,
            interval: Duration::from_millis(60_000),
            connect_timeout: Duration::from_millis(5_000),
            read_timeout: Duration::from_millis(5_000),
        }
    }
}

#[derive(Default)]
struct PingSnapshot {
    attempt_count: u64,
    success_count: u64,
    last_attempt_at: Option<SystemTime>,
    last_success_at: Option<SystemTime>,
    last_failure_at: Option<SystemTime>,
    last_error: Option<String>,
}

struct PingTask {
    cancel: watch::Sender<bool>,
    task: JoinHandle<()>,
    snapshot: Arc<Mutex<PingSnapshot>>,
}

pub struct LegacyPingAgent {
    config: LegacyPingConfig,
    sessions: Mutex<HashMap<String, PingTask>>,
}

impl LegacyPingAgent {
    pub fn new(config: LegacyPingConfig) -> Self {
        Self {
            config,
            sessions: Mutex::new(HashMap::new()),
        }
    }
}

impl SessionKeepAlivePort for LegacyPingAgent {
    fn start(&self, session_id: &str, host: &str) -> Result<bool, KeepAliveError> {
        if !self.config.enabled {
            return Ok(false);
        }
        let mut sessions = self.sessions.lock().map_err(|_| KeepAliveError::Start)?;
        if sessions.contains_key(session_id) {
            return Ok(true);
        }

        let config = self.config;
        let session_id = session_id.to_owned();
        let task_session_id = session_id.clone();
        let host = host.to_owned();
        let snapshot = Arc::new(Mutex::new(PingSnapshot::default()));
        let task_snapshot = Arc::clone(&snapshot);
        let (cancel, mut cancelled) = watch::channel(false);
        let task = tokio::runtime::Handle::try_current()
            .map_err(|_| KeepAliveError::Start)?
            .spawn(async move {
                loop {
                    if *cancelled.borrow() {
                        break;
                    }
                    if let Ok(mut state) = task_snapshot.lock() {
                        state.attempt_count += 1;
                        state.last_attempt_at = Some(SystemTime::now());
                    }

                    let ping = ping_once(&host, &task_session_id, config);
                    let result = tokio::select! {
                        _ = cancelled.changed() => break,
                        result = ping => result,
                    };
                    if let Ok(mut state) = task_snapshot.lock() {
                        match result {
                            Ok(()) => {
                                state.success_count += 1;
                                state.last_success_at = Some(SystemTime::now());
                                state.last_error = None;
                            }
                            Err(error) => {
                                state.last_failure_at = Some(SystemTime::now());
                                state.last_error = Some(error);
                            }
                        }
                    }

                    tokio::select! {
                        _ = cancelled.changed() => break,
                        _ = sleep(config.interval) => {}
                    }
                }
            });
        sessions.insert(
            session_id,
            PingTask {
                cancel,
                task,
                snapshot,
            },
        );
        Ok(true)
    }

    fn stop(&self, session_id: &str) -> Result<(), KeepAliveError> {
        let task = self
            .sessions
            .lock()
            .map_err(|_| KeepAliveError::Stop)?
            .remove(session_id);
        if let Some(task) = task {
            let _ = task.cancel.send(true);
            task.task.abort();
        }
        Ok(())
    }

    fn status(&self, session_id: &str) -> SessionKeepAliveStatus {
        let sessions = match self.sessions.lock() {
            Ok(sessions) => sessions,
            Err(_) => return inactive_status(self.config.interval),
        };
        let Some(task) = sessions.get(session_id) else {
            return inactive_status(self.config.interval);
        };
        let active = !task.task.is_finished();
        let state = match task.snapshot.lock() {
            Ok(state) => state,
            Err(_) => return inactive_status(self.config.interval),
        };
        SessionKeepAliveStatus {
            active,
            interval_ms: self.config.interval.as_millis().min(u64::MAX as u128) as u64,
            attempt_count: state.attempt_count,
            success_count: state.success_count,
            last_attempt_at: state.last_attempt_at,
            last_success_at: state.last_success_at,
            last_failure_at: state.last_failure_at,
            last_error: state.last_error.clone(),
        }
    }
}

impl Drop for LegacyPingAgent {
    fn drop(&mut self) {
        if let Ok(sessions) = self.sessions.get_mut() {
            for (_, task) in sessions.drain() {
                let _ = task.cancel.send(true);
                task.task.abort();
            }
        }
    }
}

fn inactive_status(interval: Duration) -> SessionKeepAliveStatus {
    SessionKeepAliveStatus {
        active: false,
        interval_ms: interval.as_millis().min(u64::MAX as u128) as u64,
        attempt_count: 0,
        success_count: 0,
        last_attempt_at: None,
        last_success_at: None,
        last_failure_at: None,
        last_error: None,
    }
}

async fn ping_once(host: &str, session_id: &str, config: LegacyPingConfig) -> Result<(), String> {
    let mut stream = timeout(
        config.connect_timeout,
        TcpStream::connect((host, config.port)),
    )
    .await
    .map_err(|_| "connect timeout".to_owned())?
    .map_err(|_| "connect failed".to_owned())?;
    stream
        .set_nodelay(true)
        .map_err(|_| "TCP option failed".to_owned())?;
    let message = format!("PING {session_id} HTTP/1.0\r\n");
    stream
        .write_all(message.as_bytes())
        .await
        .map_err(|_| "ping send failed".to_owned())?;
    stream
        .flush()
        .await
        .map_err(|_| "ping send failed".to_owned())?;

    let mut response = Vec::new();
    let mut buffer = [0; 512];
    loop {
        let count = timeout(config.read_timeout, stream.read(&mut buffer))
            .await
            .map_err(|_| "ping read timeout".to_owned())?
            .map_err(|_| "ping read failed".to_owned())?;
        if count == 0 || response.ends_with(RESPONSE_END) {
            return Ok(());
        }
        response.extend_from_slice(&buffer[..count]);
        if response.ends_with(RESPONSE_END) {
            return Ok(());
        }
    }
}
