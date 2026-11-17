//! Outbound asynchronous TCP communication with the legacy ERP.

mod bot_gateway;
mod ping_agent;

pub use bot_gateway::TcpBotGateway;
pub use ping_agent::{LegacyPingAgent, LegacyPingConfig};

use std::{sync::Arc, time::Duration};

use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    net::{TcpSocket, TcpStream, ToSocketAddrs, lookup_host},
    time::timeout,
};

use crate::{
    application::ports::{crypto::CryptoPort, packer::PackerPort},
    domain::protocol::{
        PACK_END, PROTO_FINALIZER, PROTO_SEPARATOR, START_MESSAGE, decode_proto_text,
        encode_proto_text,
    },
};

#[derive(Clone, Copy, Debug)]
pub struct TcpSettings {
    pub connect_timeout: Duration,
    pub socket_timeout: Duration,
    pub send_buffer_size: u32,
    pub receive_buffer_size: usize,
    pub keep_alive: bool,
    pub tcp_no_delay: bool,
    pub traffic_class: u32,
    pub linger: Option<Duration>,
}

impl Default for TcpSettings {
    fn default() -> Self {
        Self {
            connect_timeout: Duration::from_millis(5_000),
            socket_timeout: Duration::from_millis(60_000),
            send_buffer_size: 4_096,
            receive_buffer_size: 4_096,
            keep_alive: true,
            tcp_no_delay: true,
            traffic_class: 8,
            linger: None,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum TcpCommunicationError {
    #[error("failed to connect to ERP TCP server")]
    Connect(#[source] std::io::Error),
    #[error("could not resolve ERP TCP server address")]
    Resolve(#[source] std::io::Error),
    #[error("could not resolve ERP TCP server address")]
    NoAddress,
    #[error("connection to ERP TCP server timed out")]
    ConnectTimeout,
    #[error("failed to send TCP message")]
    Send(#[source] std::io::Error),
    #[error("sending TCP message timed out")]
    SendTimeout,
    #[error("failed to receive TCP message")]
    Receive(#[source] std::io::Error),
    #[error("receiving TCP message timed out")]
    ReceiveTimeout,
    #[error("TCP stream closed before finalizer")]
    UnexpectedEof,
    #[error("failed to unpack TCP message")]
    Unpack(#[from] crate::application::ports::packer::PackerError),
}

pub struct TcpCommunication {
    crypto: Arc<dyn CryptoPort>,
    packer: Arc<dyn PackerPort>,
    settings: TcpSettings,
}

impl TcpCommunication {
    pub fn new(
        crypto: Arc<dyn CryptoPort>,
        packer: Arc<dyn PackerPort>,
        settings: TcpSettings,
    ) -> Self {
        Self {
            crypto,
            packer,
            settings,
        }
    }

    pub async fn connect<A: ToSocketAddrs>(
        &self,
        address: A,
    ) -> Result<TcpStream, TcpCommunicationError> {
        timeout(self.settings.connect_timeout, async {
            let mut addresses = lookup_host(address)
                .await
                .map_err(TcpCommunicationError::Resolve)?;
            let first = addresses.next().ok_or(TcpCommunicationError::NoAddress)?;
            let socket = if first.is_ipv4() {
                TcpSocket::new_v4()
            } else {
                TcpSocket::new_v6()
            }
            .map_err(TcpCommunicationError::Connect)?;
            socket
                .set_nodelay(self.settings.tcp_no_delay)
                .map_err(TcpCommunicationError::Connect)?;
            socket
                .set_keepalive(self.settings.keep_alive)
                .map_err(TcpCommunicationError::Connect)?;
            socket
                .set_send_buffer_size(self.settings.send_buffer_size)
                .map_err(TcpCommunicationError::Connect)?;
            socket
                .set_recv_buffer_size(
                    self.settings.receive_buffer_size.min(u32::MAX as usize) as u32
                )
                .map_err(TcpCommunicationError::Connect)?;
            set_legacy_linger(&socket, self.settings.linger)
                .map_err(TcpCommunicationError::Connect)?;
            if first.is_ipv4() {
                socket
                    .set_tos_v4(self.settings.traffic_class)
                    .map_err(TcpCommunicationError::Connect)?;
            }
            socket
                .connect(first)
                .await
                .map_err(TcpCommunicationError::Connect)
        })
        .await
        .map_err(|_| TcpCommunicationError::ConnectTimeout)?
    }

    pub async fn send<W: AsyncWrite + Unpin>(
        &self,
        writer: &mut W,
        message: &str,
    ) -> Result<(), TcpCommunicationError> {
        let encrypted = self.crypto.apply_tcp_xor(&encode_proto_text(message));
        timeout(self.settings.socket_timeout, async {
            writer.write_all(&encrypted).await?;
            writer.flush().await
        })
        .await
        .map_err(|_| TcpCommunicationError::SendTimeout)?
        .map_err(TcpCommunicationError::Send)
    }

    pub async fn receive<R: AsyncRead + Unpin>(
        &self,
        reader: &mut R,
    ) -> Result<String, TcpCommunicationError> {
        let finalizer = encode_proto_text(PROTO_FINALIZER);
        let packed_end = encode_proto_text(PACK_END);
        let mut buffer = vec![0; self.settings.receive_buffer_size.max(1)];
        let mut received = Vec::new();

        loop {
            let count = timeout(self.settings.socket_timeout, reader.read(&mut buffer))
                .await
                .map_err(|_| TcpCommunicationError::ReceiveTimeout)?
                .map_err(TcpCommunicationError::Receive)?;
            if count == 0 {
                return Err(TcpCommunicationError::UnexpectedEof);
            }
            received.extend(self.crypto.apply_tcp_xor(&buffer[..count]));
            if received.ends_with(&finalizer) || contains(&received, &packed_end) {
                let unpacked = self.packer.unpack(&received)?;
                return Ok(decode_proto_text(&unpacked));
            }
        }
    }

    pub fn build_handshake_message(&self, message: &str) -> String {
        format!("{START_MESSAGE}{PROTO_SEPARATOR}?0{PROTO_SEPARATOR}{message}{PROTO_FINALIZER}")
    }

    pub fn build_session_message(&self, session_id: &str, message: &str) -> String {
        format!("{session_id}{PROTO_SEPARATOR}?0{PROTO_SEPARATOR}{message}{PROTO_FINALIZER}")
    }
}

// SO_LINGER is part of the Java socket configuration contract; Tokio marks this
// wrapper deprecated because closing can block a runtime worker when enabled.
#[allow(deprecated)]
fn set_legacy_linger(socket: &TcpSocket, linger: Option<Duration>) -> std::io::Result<()> {
    socket.set_linger(linger)
}

fn contains(value: &[u8], pattern: &[u8]) -> bool {
    !pattern.is_empty() && value.windows(pattern.len()).any(|window| window == pattern)
}

#[cfg(test)]
mod tests {
    use tokio::io::{AsyncWriteExt, duplex};

    use crate::{
        adapters::{crypto::CryptoAdapter, protocol::DeflatePackerAdapter},
        application::ports::{crypto::CryptoPort, packer::PackerPort},
    };

    use super::*;

    fn communication() -> TcpCommunication {
        TcpCommunication::new(
            Arc::new(CryptoAdapter::new("1234567890123456")) as Arc<dyn CryptoPort>,
            Arc::new(DeflatePackerAdapter) as Arc<dyn PackerPort>,
            TcpSettings {
                receive_buffer_size: 8,
                ..TcpSettings::default()
            },
        )
    }

    #[tokio::test]
    async fn sends_xor_encoded_latin1_bytes() {
        let communication = communication();
        let (mut writer, mut reader) = duplex(32);
        communication
            .send(&mut writer, "ABC")
            .await
            .expect("send succeeds");
        let mut bytes = [0; 3];
        tokio::io::AsyncReadExt::read_exact(&mut reader, &mut bytes)
            .await
            .expect("read sent bytes");
        assert_eq!(bytes, [4, 7, 6]);
    }

    #[tokio::test]
    async fn receives_through_finalizer_and_decrypts_xor() {
        let communication = communication();
        let plaintext = format!("SESSION{PROTO_SEPARATOR}OK{PROTO_FINALIZER}");
        let encrypted =
            CryptoAdapter::new("1234567890123456").apply_tcp_xor(&encode_proto_text(&plaintext));
        let (mut writer, mut reader) = duplex(64);
        writer.write_all(&encrypted).await.expect("write fixture");
        assert_eq!(
            communication
                .receive(&mut reader)
                .await
                .expect("receive succeeds"),
            plaintext
        );
    }

    #[tokio::test]
    async fn receives_and_unpacks_legacy_pack_envelope() {
        let communication = communication();
        let message = b"PROTO;".repeat(40);
        let packed = DeflatePackerAdapter.pack(&message).expect("pack fixture");
        let encrypted = CryptoAdapter::new("1234567890123456").apply_tcp_xor(&packed);
        let (mut writer, mut reader) = duplex(encrypted.len());
        writer.write_all(&encrypted).await.expect("write fixture");
        assert_eq!(
            communication
                .receive(&mut reader)
                .await
                .expect("receive succeeds"),
            String::from_utf8(message).expect("ASCII fixture")
        );
    }

    #[tokio::test]
    async fn reports_eof_before_a_complete_message() {
        let communication = communication();
        let (writer, mut reader) = duplex(16);
        drop(writer);
        assert!(matches!(
            communication.receive(&mut reader).await,
            Err(TcpCommunicationError::UnexpectedEof)
        ));
    }

    #[tokio::test]
    async fn applies_a_read_timeout_when_peer_stays_silent() {
        let mut communication = communication();
        communication.settings.socket_timeout = Duration::from_millis(10);
        let (_writer, mut reader) = duplex(16);
        assert!(matches!(
            communication.receive(&mut reader).await,
            Err(TcpCommunicationError::ReceiveTimeout)
        ));
    }

    #[test]
    fn builds_legacy_handshake_and_session_envelopes() {
        let communication = communication();
        assert_eq!(
            communication.build_handshake_message("HELLO"),
            format!("XONGAS{PROTO_SEPARATOR}?0{PROTO_SEPARATOR}HELLO{PROTO_FINALIZER}")
        );
        assert_eq!(
            communication.build_session_message("S1", "HELLO"),
            format!("S1{PROTO_SEPARATOR}?0{PROTO_SEPARATOR}HELLO{PROTO_FINALIZER}")
        );
    }
}
