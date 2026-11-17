use std::{
    env,
    net::{AddrParseError, SocketAddr},
    str::FromStr,
    time::Duration,
};

use crate::adapters::tcp::{LegacyPingConfig, TcpSettings};

#[derive(Debug, thiserror::Error)]
pub enum ConfigurationError {
    #[error("required environment variable `cipher` is missing or empty")]
    MissingCipher,
    #[error("environment variable `cipher` must encode to 16, 24, or 32 bytes")]
    InvalidCipherKeyLength,
    #[error("invalid value for `NUONE_HTTP_BIND`")]
    InvalidHttpBind(#[source] AddrParseError),
    #[error("invalid value for environment variable `{0}`")]
    InvalidValue(&'static str),
}

pub struct Settings {
    pub http_bind: SocketAddr,
    pub cipher_key: String,
    pub bot_version: String,
    pub mode_suffix: String,
    pub development_mode: bool,
    pub init_loop_timeout: Duration,
    pub tcp: TcpSettings,
    pub legacy_ping: LegacyPingConfig,
}

impl Settings {
    pub fn from_env() -> Result<Self, ConfigurationError> {
        let cipher_key = env::var("cipher").unwrap_or_default();
        if cipher_key.is_empty() {
            return Err(ConfigurationError::MissingCipher);
        }
        if !matches!(cipher_key.len(), 16 | 24 | 32) {
            return Err(ConfigurationError::InvalidCipherKeyLength);
        }

        let bind = env::var("NUONE_HTTP_BIND").unwrap_or_else(|_| "0.0.0.0:8080".to_owned());
        let http_bind = bind.parse().map_err(ConfigurationError::InvalidHttpBind)?;

        let tcp_linger = parse_env("NUONE_TCP_LINGER", false)?;
        let linger_seconds: u64 = parse_env("NUONE_TCP_LINGER_DURATION_SECONDS", 10)?;
        let tcp = TcpSettings {
            connect_timeout: Duration::from_millis(parse_positive(
                "NUONE_TCP_CONNECT_TIMEOUT_MS",
                5_000u64,
            )?),
            socket_timeout: Duration::from_millis(parse_positive(
                "NUONE_TCP_SOCKET_TIMEOUT_MS",
                60_000u64,
            )?),
            send_buffer_size: parse_positive("NUONE_TCP_SEND_BUFFER_SIZE", 4_096u32)?,
            receive_buffer_size: parse_positive("NUONE_TCP_RECEIVE_BUFFER_SIZE", 4_096usize)?,
            keep_alive: parse_env("NUONE_TCP_KEEP_ALIVE", true)?,
            tcp_no_delay: parse_env("NUONE_TCP_NO_DELAY", true)?,
            traffic_class: parse_env("NUONE_TCP_TRAFFIC_CLASS", 8u32)?,
            linger: tcp_linger.then(|| Duration::from_secs(linger_seconds)),
        };
        let legacy_ping = LegacyPingConfig {
            enabled: parse_env("NUONE_LEGACY_PING_ENABLED", true)?,
            port: parse_positive("NUONE_LEGACY_PING_PORT", 4_081u16)?,
            interval: Duration::from_millis(parse_positive(
                "NUONE_LEGACY_PING_INTERVAL_MS",
                60_000u64,
            )?),
            connect_timeout: Duration::from_millis(parse_positive(
                "NUONE_LEGACY_PING_CONNECT_TIMEOUT_MS",
                5_000u64,
            )?),
            read_timeout: Duration::from_millis(parse_positive(
                "NUONE_LEGACY_PING_READ_TIMEOUT_MS",
                5_000u64,
            )?),
        };

        Ok(Self {
            http_bind,
            cipher_key,
            bot_version: env::var("NUONE_BOT_VERSION").unwrap_or_else(|_| "7000".to_owned()),
            mode_suffix: env::var("NUONE_MODE_SUFFIX").unwrap_or_default(),
            development_mode: parse_env("NUONE_DEVELOPMENT_MODE", false)?,
            init_loop_timeout: Duration::from_millis(parse_positive(
                "NUONE_INIT_LOOP_TIMEOUT_MS",
                30_000u64,
            )?),
            tcp,
            legacy_ping,
        })
    }
}

fn parse_env<T: FromStr>(name: &'static str, default: T) -> Result<T, ConfigurationError> {
    match env::var(name) {
        Ok(value) => value
            .parse()
            .map_err(|_| ConfigurationError::InvalidValue(name)),
        Err(env::VarError::NotPresent) => Ok(default),
        Err(env::VarError::NotUnicode(_)) => Err(ConfigurationError::InvalidValue(name)),
    }
}

fn parse_positive<T>(name: &'static str, default: T) -> Result<T, ConfigurationError>
where
    T: FromStr + PartialOrd + From<u8>,
{
    let value = parse_env(name, default)?;
    if value < T::from(1) {
        return Err(ConfigurationError::InvalidValue(name));
    }
    Ok(value)
}
