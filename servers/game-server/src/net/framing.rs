use super::crypto::CipherState;
use perlica_proto::{CsHead, prost::Message};
use std::io;
use tokio::io::{AsyncRead, AsyncReadExt};

pub(super) struct InboundPacket {
    pub cmd_id: i32,
    pub body: Vec<u8>,
    pub client_seq_id: u64,
}

/// Retains consumed bytes so a cancelled `select!` read resumes mid-frame.
pub(super) struct PacketReader<R> {
    reader: R,
    prefix: [u8; 3],
    prefix_filled: usize,
    payload: Vec<u8>,
    payload_filled: usize,
}

impl<R: AsyncRead + Unpin> PacketReader<R> {
    pub fn new(reader: R) -> Self {
        Self {
            reader,
            prefix: [0; 3],
            prefix_filled: 0,
            payload: Vec::new(),
            payload_filled: 0,
        }
    }

    /// `None` is EOF at a frame boundary, EOF inside a frame is an error.
    pub async fn read_packet(
        &mut self,
        cipher: &mut CipherState,
    ) -> io::Result<Option<InboundPacket>> {
        while self.prefix_filled < self.prefix.len() {
            let count = self
                .reader
                .read(&mut self.prefix[self.prefix_filled..])
                .await?;
            if count == 0 {
                return if self.prefix_filled == 0 {
                    Ok(None)
                } else {
                    Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "partial frame prefix",
                    ))
                };
            }
            self.prefix_filled += count;
        }

        let head_size = usize::from(self.prefix[0]);
        if head_size == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "empty frame header",
            ));
        }
        let body_size = usize::from(u16::from_le_bytes([self.prefix[1], self.prefix[2]]));
        self.payload.resize(head_size + body_size, 0);
        while self.payload_filled < self.payload.len() {
            let count = self
                .reader
                .read(&mut self.payload[self.payload_filled..])
                .await?;
            if count == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "partial frame payload",
                ));
            }
            self.payload_filled += count;
        }

        // No read-ahead: login arms its cipher before the next frame decodes.
        cipher.apply(&mut self.payload);
        let head = CsHead::decode(&self.payload[..head_size])
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        let body = self.payload.split_off(head_size);
        self.payload.clear();
        self.prefix_filled = 0;
        self.payload_filled = 0;
        Ok(Some(InboundPacket {
            cmd_id: head.msgid,
            body,
            client_seq_id: head.up_seqid,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::crypto::SessionKeys;
    use std::time::Duration;
    use tokio::io::{AsyncWriteExt, duplex};

    fn frame(sequence: u64, body: &[u8], cipher: &mut CipherState) -> Vec<u8> {
        let header = CsHead {
            msgid: 42,
            up_seqid: sequence,
            ..Default::default()
        }
        .encode_to_vec();
        let mut bytes = vec![u8::try_from(header.len()).unwrap()];
        bytes.extend_from_slice(&u16::try_from(body.len()).unwrap().to_le_bytes());
        bytes.extend_from_slice(&header);
        bytes.extend_from_slice(body);
        cipher.apply(&mut bytes[3..]);
        bytes
    }

    fn arm(cipher: &mut CipherState) {
        cipher.arm(
            SessionKeys {
                key: [7; 32],
                nonce: [9; 12],
            }
            .cipher(),
        );
        cipher.activate_pending();
    }

    #[tokio::test(start_paused = true)]
    async fn every_fragment_boundary_survives_timer_cancellation_and_cipher_continuation() {
        for encrypted in [false, true] {
            let frame_len = frame(17, b"first-body", &mut CipherState::new()).len();
            for split in 1..frame_len {
                let (mut peer, socket) = duplex(256);
                let mut reader = PacketReader::new(socket);
                let mut inbound = CipherState::new();
                let mut outbound = CipherState::new();
                if encrypted {
                    arm(&mut inbound);
                    arm(&mut outbound);
                }
                let first = frame(17, b"first-body", &mut outbound);
                let second = frame(18, b"second-body", &mut outbound);
                peer.write_all(&first[..split]).await.unwrap();
                for _ in 0..3 {
                    tokio::select! {
                        biased;
                        result = reader.read_packet(&mut inbound) => {
                            panic!("partial frame completed: {:?}", result.err());
                        }
                        () = tokio::time::sleep(Duration::from_millis(16)) => {}
                    }
                }
                peer.write_all(&first[split..]).await.unwrap();
                peer.write_all(&second).await.unwrap();
                for (sequence, body) in [
                    (17, b"first-body".as_slice()),
                    (18, b"second-body".as_slice()),
                ] {
                    let packet = tokio::time::timeout(
                        Duration::from_secs(1),
                        reader.read_packet(&mut inbound),
                    )
                    .await
                    .unwrap()
                    .unwrap()
                    .unwrap();
                    assert_eq!((packet.cmd_id, packet.client_seq_id), (42, sequence));
                    assert_eq!(packet.body, body);
                }
                peer.shutdown().await.unwrap();
                assert!(reader.read_packet(&mut inbound).await.unwrap().is_none());
            }
        }
    }

    #[tokio::test]
    async fn coalesced_plaintext_and_encrypted_frames_observe_handshake_boundary() {
        let (mut peer, socket) = duplex(256);
        let mut outbound = CipherState::new();
        let mut bytes = frame(1, b"login", &mut outbound);
        arm(&mut outbound);
        bytes.extend(frame(2, b"after-login", &mut outbound));
        peer.write_all(&bytes).await.unwrap();
        let mut reader = PacketReader::new(socket);
        let mut inbound = CipherState::new();
        assert_eq!(
            reader
                .read_packet(&mut inbound)
                .await
                .unwrap()
                .unwrap()
                .body,
            b"login"
        );
        arm(&mut inbound);
        assert_eq!(
            reader
                .read_packet(&mut inbound)
                .await
                .unwrap()
                .unwrap()
                .body,
            b"after-login"
        );
    }

    #[tokio::test]
    async fn truncated_frames_are_distinct_from_boundary_eof() {
        let bytes = frame(1, b"payload", &mut CipherState::new());
        for end in 1..bytes.len() {
            let mut reader = PacketReader::new(&bytes[..end]);
            let error = reader
                .read_packet(&mut CipherState::new())
                .await
                .err()
                .unwrap();
            assert_eq!(error.kind(), io::ErrorKind::UnexpectedEof);
        }
        let mut reader = PacketReader::new(b"".as_slice());
        assert!(
            reader
                .read_packet(&mut CipherState::new())
                .await
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn maximum_body_is_retained_without_length_truncation() {
        let body = vec![0x5a; usize::from(u16::MAX)];
        let bytes = frame(1, &body, &mut CipherState::new());
        let mut reader = PacketReader::new(bytes.as_slice());
        assert_eq!(
            reader
                .read_packet(&mut CipherState::new())
                .await
                .unwrap()
                .unwrap()
                .body,
            body
        );
    }

    #[tokio::test]
    async fn malformed_headers_fail_explicitly() {
        for bytes in [b"\0\0\0".as_slice(), b"\x01\0\0\xff".as_slice()] {
            let mut reader = PacketReader::new(bytes);
            let error = reader
                .read_packet(&mut CipherState::new())
                .await
                .err()
                .unwrap();
            assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        }
    }
}
