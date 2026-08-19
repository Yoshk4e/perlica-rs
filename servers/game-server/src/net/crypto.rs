//! Transport-layer encryption for the game protocol.
//!
//! # Wire model
//!
//! Every frame on the wire looks like:
//!
//! ```text
//! [u8 head_size][u16 body_size (LE)][head bytes][body bytes]
//! ```
//!
//! The 3-byte size prefix (`head_size` + `body_size`) is ALWAYS transmitted in the
//! clear. After the handshake completes, the `head` and `body` regions are XORed
//! (concatenated as `head ‖ body`) against a per-direction ChaCha20 keystream
//! (RFC 7539, 32-byte key, 12-byte nonce). Peer and server share the same
//! key/nonce, but each direction keeps its own cipher instance so counters never
//! collide and keystream is never reused.
//!
//! IMPORTANT: the client's cipher starts generating keystream from block
//! counter 1, not the RustCrypto `chacha20` crate's default of counter 0.
//! `SessionKeys::cipher` seeks each fresh instance forward one block to
//! compensate — skip that and every byte after `SC_LOGIN` decrypts to noise.
//!
//! # Handshake (server side)
//!
//! 1. Pre-login: everything plaintext.
//! 2. `CS_LOGIN` arrives with `client_public_key` = client's RSA public key in PEM.
//! 3. Server generates 32B ChaCha20 key + 12B nonce (both cryptographically random).
//! 4. Server RSA-PKCS1v1.5-encrypts the 32B key with the client public key.
//! 5. Server sends `SC_LOGIN` **still plaintext**, with
//!    `server_public_key = RSA(ChaCha20 key)`,
//!    `server_encryp_nonce = 12B nonce`,
//!    `is_enc = true`.
//! 6. The `SC_LOGIN` frame itself is the last plaintext outbound frame; every
//!    subsequent outbound frame is XORed. Symmetrically, every subsequent inbound
//!    frame is XORed.

use chacha20::ChaCha20;
use chacha20::cipher::{KeyIvInit, StreamCipher, StreamCipherSeek};
use rand::RngCore;
use rand::rngs::OsRng;
use rsa::pkcs1::DecodeRsaPublicKey;
use rsa::pkcs8::DecodePublicKey;
use rsa::{Pkcs1v15Encrypt, RsaPublicKey};

/// ChaCha20 key length in bytes (RFC 7539).
pub const CHACHA_KEY_LEN: usize = 32;
/// ChaCha20 nonce length in bytes (RFC 7539).
pub const CHACHA_NONCE_LEN: usize = 12;
/// ChaCha20 block size in bytes. The client's cipher starts generating
/// keystream from block counter 1 rather than 0 — see `SessionKeys::cipher`.
const CHACHA_BLOCK_LEN: u32 = 64;

/// Freshly generated symmetric key material for a single session.
///
/// `key` is the raw 32-byte ChaCha20 key (kept only long enough to RSA-encrypt
/// it for the client and to seed the per-direction cipher instances). `nonce`
/// is transmitted plaintext in `SC_LOGIN.server_encryp_nonce`.
#[derive(Clone)]
pub struct SessionKeys {
    pub key: [u8; CHACHA_KEY_LEN],
    pub nonce: [u8; CHACHA_NONCE_LEN],
}

impl SessionKeys {
    /// Generates key/nonce using the OS CSPRNG.
    pub fn generate() -> Self {
        let mut key = [0u8; CHACHA_KEY_LEN];
        let mut nonce = [0u8; CHACHA_NONCE_LEN];
        OsRng.fill_bytes(&mut key);
        OsRng.fill_bytes(&mut nonce);
        Self { key, nonce }
    }

    /// Builds a fresh ChaCha20 cipher instance keyed with this session's material.
    /// A separate instance is created per direction, but note this crate's
    /// `ChaCha20::new` starts the block counter at 0 (the RFC 8439 default) —
    /// the client's implementation explicitly starts at counter 1 instead, so
    /// we seek forward one block (64 bytes) here to match. Without this, our
    /// keystream is offset by one block from the client's from the very first
    /// encrypted byte, every frame after `SC_LOGIN` fails to decrypt into
    /// anything sane, and the connection dies immediately post-login.
    pub fn cipher(&self) -> ChaCha20 {
        let mut cipher = ChaCha20::new(&self.key.into(), &self.nonce.into());
        cipher
            .try_seek(CHACHA_BLOCK_LEN)
            .expect("seeking to block 1 is always in range for a fresh cipher");
        cipher
    }
}

/// Errors that can occur while parsing the client RSA public key or encrypting the
/// symmetric key with it.
#[derive(Debug, thiserror::Error)]
pub enum HandshakeError {
    #[error("client public key was not valid RSA PEM: {0}")]
    BadPublicKey(String),
    #[error("RSA encryption failed: {0}")]
    RsaEncrypt(#[from] rsa::Error),
}

/// Parses `pem` as an RSA public key (accepts both PKCS#1 `-----BEGIN RSA PUBLIC KEY-----`
/// and SPKI `-----BEGIN PUBLIC KEY-----` framings) and RSA-PKCS1v1.5-encrypts `key`.
///
/// The output is the ciphertext to place in `ScLogin.server_public_key` (the field
/// name is a client-side legacy; it actually carries the encrypted ChaCha20 key).
pub fn rsa_encrypt_key(pem: &[u8], key: &[u8; CHACHA_KEY_LEN]) -> Result<Vec<u8>, HandshakeError> {
    let pem_str = std::str::from_utf8(pem)
        .map_err(|e| HandshakeError::BadPublicKey(format!("pem not utf-8: {e}")))?;

    // Try PKCS#1 first (`RSA PUBLIC KEY`), then fall back to SPKI (`PUBLIC KEY`).
    let pubkey = RsaPublicKey::from_pkcs1_pem(pem_str)
        .or_else(|_| RsaPublicKey::from_public_key_pem(pem_str))
        .map_err(|e| HandshakeError::BadPublicKey(e.to_string()))?;

    let mut rng = OsRng;
    let ct = pubkey.encrypt(&mut rng, Pkcs1v15Encrypt, key)?;
    Ok(ct)
}

/// Per-direction cipher state.
///
/// Starts in `Plaintext`. Once `arm` is called with fresh keys, `pending` holds a
/// cipher instance that becomes active AFTER the next frame is processed
/// (activation happens via [`CipherState::activate_pending`]). This gives us the
/// "SC_LOGIN itself is plaintext, everything after is encrypted" semantics without
/// any conditional logic in handlers.
pub struct CipherState {
    active: Option<ChaCha20>,
    pending: Option<ChaCha20>,
}

impl CipherState {
    pub fn new() -> Self {
        Self {
            active: None,
            pending: None,
        }
    }

    /// Whether encryption is currently applied to frames.
    #[inline]
    pub fn is_encrypted(&self) -> bool {
        self.active.is_some()
    }

    /// Queues a cipher to become active on the NEXT call to `activate_pending`.
    /// Overwrites any previously pending cipher (only login should call this and
    /// only once per session, so overwrite is defensive).
    pub fn arm(&mut self, cipher: ChaCha20) {
        self.pending = Some(cipher);
    }

    /// Promotes the pending cipher to active (called after the plaintext SC_LOGIN
    /// frame has been fully written, or after the plaintext CS_LOGIN frame has
    /// been fully consumed).
    pub fn activate_pending(&mut self) {
        if let Some(c) = self.pending.take() {
            self.active = Some(c);
        }
    }

    /// XORs `buf` in place with the keystream if a cipher is active; no-op otherwise.
    /// Applied to `head ‖ body` only — the 3-byte size prefix stays plaintext.
    #[inline]
    pub fn apply(&mut self, buf: &mut [u8]) {
        if let Some(c) = self.active.as_mut() {
            c.apply_keystream(buf);
        }
    }
}

impl Default for CipherState {
    fn default() -> Self {
        Self::new()
    }
}
