//! Persistent static X25519 identity + human-readable address derivation.
//!
//! The private key never leaves the device, is never logged, and never enters a
//! signaling blob — only the public key is shared. On Unix the key file is
//! written `0600`; on Windows it inherits the per-user AppData ACL.

use std::fs;
use std::io;
use std::path::Path;

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use snow::Builder;
use zeroize::Zeroizing;

use super::NOISE_PARAMS;

const STORE_VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredIdentity {
    version: u32,
    private_key: String,
    public_key: String,
}

/// A node's static keypair. The private key is held in a zeroizing buffer so it
/// is wiped from memory on drop.
pub struct Identity {
    private: Zeroizing<Vec<u8>>,
    public: Vec<u8>,
}

impl Identity {
    /// Load the identity from `path`, generating and persisting a new one on
    /// first run. If an existing file is unreadable/corrupt, it is backed up to
    /// `<path>.corrupt` and a fresh identity is generated so the app still
    /// starts (a corrupt key file is already unrecoverable).
    pub fn load_or_generate(path: &Path) -> io::Result<Self> {
        if path.exists() {
            match Self::load(path) {
                Ok(identity) => return Ok(identity),
                Err(_corrupt) => {
                    let _ = fs::rename(path, path.with_extension("corrupt"));
                }
            }
        }
        let identity = Self::generate()?;
        identity.save(path)?;
        Ok(identity)
    }

    pub(crate) fn generate() -> io::Result<Self> {
        let params = NOISE_PARAMS.parse().map_err(other)?;
        let keypair = Builder::new(params).generate_keypair().map_err(other)?;
        Ok(Self {
            private: Zeroizing::new(keypair.private),
            public: keypair.public,
        })
    }

    fn load(path: &Path) -> io::Result<Self> {
        let data = fs::read(path)?;
        let stored: StoredIdentity = serde_json::from_slice(&data)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("identity.json: {e}")))?;
        let private = Zeroizing::new(
            STANDARD
                .decode(stored.private_key.trim())
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("private key: {e}")))?,
        );
        let public = STANDARD
            .decode(stored.public_key.trim())
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("public key: {e}")))?;
        if private.len() != 32 || public.len() != 32 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "identity key has unexpected length",
            ));
        }
        Ok(Self { private, public })
    }

    fn save(&self, path: &Path) -> io::Result<()> {
        let stored = StoredIdentity {
            version: STORE_VERSION,
            private_key: STANDARD.encode(self.private.as_slice()),
            public_key: STANDARD.encode(&self.public),
        };
        let json = serde_json::to_vec_pretty(&stored).map_err(other)?;
        write_secret(path, &json)
    }

    /// The static private key bytes (for the Noise handshake).
    pub fn private(&self) -> &[u8] {
        self.private.as_slice()
    }

    /// Base64 of the public key — the canonical identifier used in signaling.
    pub fn public_b64(&self) -> String {
        STANDARD.encode(&self.public)
    }

    /// 64-bit human-readable fingerprint of this node's public key for display
    /// and out-of-band verification, e.g. `PC-3F8A-9C2D-71B0-44EE`. The full
    /// public key (see [`public_b64`]) remains the authoritative identifier.
    ///
    /// [`public_b64`]: Identity::public_b64
    pub fn peer_address(&self) -> String {
        fingerprint_of(&self.public)
    }
}

/// Derive the `PC-XXXX-XXXX-XXXX-XXXX` fingerprint of any public key (used to
/// display a peer's address consistently with our own).
pub fn fingerprint_of(public: &[u8]) -> String {
    let d = Sha256::digest(public);
    format!(
        "PC-{:02X}{:02X}-{:02X}{:02X}-{:02X}{:02X}-{:02X}{:02X}",
        d[0], d[1], d[2], d[3], d[4], d[5], d[6], d[7]
    )
}

fn other<E: std::fmt::Display>(e: E) -> io::Error {
    io::Error::other(e.to_string())
}

/// Write a secret file, owner-only (`0600`) on Unix.
fn write_secret(path: &Path, bytes: &[u8]) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(path)?;
        f.write_all(bytes)?;
        // Enforce `0600` even if the file pre-existed with looser permissions
        // (the create-time `mode` only applies to newly created files).
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
        Ok(())
    }
    #[cfg(not(unix))]
    {
        // On Windows the file inherits the per-user AppData ACL (not readable by
        // other standard users). Tightening to an explicit owner-only DACL is a
        // tracked future hardening.
        fs::write(path, bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn peer_address_is_deterministic_and_formatted() {
        let id = Identity::generate().unwrap();
        let a = id.peer_address();
        assert_eq!(a, id.peer_address());
        assert!(a.starts_with("PC-"));
        assert_eq!(a.len(), 22); // "PC-XXXX-XXXX-XXXX-XXXX"
    }

    #[test]
    fn persists_and_reloads() {
        let path = std::env::temp_dir().join(format!("pcpv_id_{}.json", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let a = Identity::load_or_generate(&path).unwrap();
        let b = Identity::load_or_generate(&path).unwrap();
        assert_eq!(a.public_b64(), b.public_b64());
        assert_eq!(a.peer_address(), b.peer_address());
        let _ = std::fs::remove_file(&path);
    }
}
