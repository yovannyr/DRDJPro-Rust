//! Secure credential storage for external providers.
//!
//! Credentials are stored encrypted in the library store
//! (SQLite + AES-GCM encryption). No plaintext secrets
//! are written to disk or exposed to the process environment.
//!
//! ## Design
//!
//! * All secrets are encrypted with AES-GCM using a device-bound key
//!   derived from OS entropy (`getrandom`).
//! * The encryption key is never persisted — it lives only in
//!   memory for the duration of a session.
//! * Provider tokens are stored per-service in a key-value map;
//!   individual keys can be rotated without touching others.
//! * All public API returns `Result` / `Option`; panics are
//!   impossible for normal failures.

use std::path::PathBuf;
use std::sync::Mutex;

use aes_gcm::{aead::Aead, Aes256Gcm, KeyInit, Nonce};
use getrandom::fill;
use rusqlite::OptionalExtension;

// ---------------------------------------------------------------------------
// Data types
// ---------------------------------------------------------------------------

/// Version tag for the encrypted format.
pub const CREDENTIAL_FORMAT_VERSION: u32 = 1;

// ---------------------------------------------------------------------------
// Encryption key management
// ---------------------------------------------------------------------------

/// 32-byte AES-GCM key, never persisted to disk.
type EncryptionKey = [u8; 32];

/// Generate a random encryption key from OS entropy.
#[cfg(not(test))]
fn generate_key() -> EncryptionKey {
    let mut key = [0u8; 32];
    fill(&mut key).expect("OS entropy unavailable");
    key
}

#[cfg(test)]
fn generate_key() -> EncryptionKey {
    // Deterministic test key for reproducibility
    [0x42u8; 32]
}

/// Encrypt plaintext with AES-GCM using the given key.
/// Returns (nonce[12] || ciphertext_with_tag[32]).
fn encrypt(key: &EncryptionKey, plaintext: &[u8]) -> Vec<u8> {
    let cipher = Aes256Gcm::new(key.into());
    let mut nonce_bytes = [0u8; 12];
    fill(&mut nonce_bytes).expect("OS entropy unavailable");
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher.encrypt(nonce, plaintext).expect("encryption failed");
    let mut result = Vec::with_capacity(12 + ciphertext.len());
    result.extend_from_slice(nonce);
    result.extend_from_slice(&ciphertext);
    result
}

/// Decrypt ciphertext produced by [`encrypt`].
fn decrypt(key: &EncryptionKey, data: &[u8]) -> Result<Vec<u8>, String> {
    if data.len() < 12 + 16 {
        return Err("Ciphertext too short".into());
    }

    let (nonce, ciphertext) = data.split_at(12);
    let nonce = Nonce::from_slice(nonce);
    let cipher = Aes256Gcm::new(key.into());
    cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| format!("Decryption failed: {e}"))
}

// ---------------------------------------------------------------------------
// Storage: SQLite-backed credential vault
// ---------------------------------------------------------------------------

const CREDENTIALS_TABLE: &str = "CREATE TABLE IF NOT EXISTS credentials_v1 (
    service TEXT PRIMARY KEY,
    version INTEGER NOT NULL DEFAULT 1,
    encrypted_blob BLOB NOT NULL
)";

/// Credential vault backed by SQLite.
pub struct Vault {
    db_path: PathBuf,
    key: EncryptionKey,
}

impl Vault {
    /// Open (or create) the credential vault at the given path.
    /// A fresh key is generated and used for all encryption.
    pub fn open(db_path: impl Into<PathBuf>) -> Result<Self, String> {
        let path = db_path.into();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create credential directory: {e}"))?;
        }

        let conn = rusqlite::Connection::open(&path)
            .map_err(|e| format!("Failed to open credential database: {e}"))?;

        conn.execute_batch(CREDENTIALS_TABLE)
            .map_err(|e| format!("Failed to create credentials table: {e}"))?;

        let key = generate_key();

        Ok(Self { db_path: path, key })
    }

    /// Store or update a credential for a service.
    pub fn store(&self, service: &str, plaintext: &[u8]) -> Result<(), String> {
        let encrypted = encrypt(&self.key, plaintext);

        let conn = rusqlite::Connection::open(&self.db_path)
            .map_err(|e| format!("Failed to open credential database: {e}"))?;

        conn.execute(
            "INSERT OR REPLACE INTO credentials_v1 (service, version, encrypted_blob)
             VALUES (?1, ?2, ?3)",
            rusqlite::params![service, CREDENTIAL_FORMAT_VERSION, encrypted,],
        )
        .map_err(|e| format!("Failed to store credential: {e}"))?;

        Ok(())
    }

    /// Retrieve and decrypt a credential for a service.
    /// Returns `None` if the service has no stored credential.
    pub fn retrieve(&self, service: &str) -> Result<Option<Vec<u8>>, String> {
        let conn = rusqlite::Connection::open(&self.db_path)
            .map_err(|e| format!("Failed to open credential database: {e}"))?;

        let row: Option<(u32, Vec<u8>)> = conn
            .query_row(
                "SELECT version, encrypted_blob FROM credentials_v1 WHERE service = ?1",
                rusqlite::params![service],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(|e| e.to_string())?;

        match row {
            Some((version, encrypted)) => {
                if version != CREDENTIAL_FORMAT_VERSION {
                    return Err(format!("Unsupported credential format version: {version}"));
                }
                let plaintext = decrypt(&self.key, &encrypted)?;
                Ok(Some(plaintext))
            }
            None => Ok(None),
        }
    }

    /// Delete a credential for a service.
    pub fn delete(&self, service: &str) -> Result<(), String> {
        let conn = rusqlite::Connection::open(&self.db_path)
            .map_err(|e| format!("Failed to open credential database: {e}"))?;

        conn.execute(
            "DELETE FROM credentials_v1 WHERE service = ?1",
            rusqlite::params![service],
        )
        .map_err(|e| format!("Failed to delete credential: {e}"))?;

        Ok(())
    }

    /// List all stored service names (no secrets).
    pub fn list_services(&self) -> Result<Vec<String>, String> {
        let conn = rusqlite::Connection::open(&self.db_path)
            .map_err(|e| format!("Failed to open credential database: {e}"))?;

        let mut statement = conn
            .prepare("SELECT service FROM credentials_v1 ORDER BY service")
            .map_err(|e| e.to_string())?;
        let rows = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())
    }

    /// Clear all credentials.
    pub fn clear(&self) -> Result<(), String> {
        let conn = rusqlite::Connection::open(&self.db_path)
            .map_err(|e| format!("Failed to open credential database: {e}"))?;

        conn.execute("DELETE FROM credentials_v1", [])
            .map_err(|e| format!("Failed to clear credentials: {e}"))?;

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// High-level API: thread-safe credential manager
// ---------------------------------------------------------------------------

/// Thread-safe credential manager shared across the application.
pub struct Manager {
    vault: Mutex<Vault>,
}

impl Manager {
    /// Create a new credential manager backed by a SQLite database at `db_path`.
    pub fn new(db_path: impl Into<PathBuf>) -> Result<Self, String> {
        let vault = Vault::open(db_path)?;
        Ok(Self {
            vault: Mutex::new(vault),
        })
    }

    /// Store an OAuth token or other credential.
    pub fn store(&self, service: &str, data: &[u8]) -> Result<(), String> {
        self.vault
            .lock()
            .map_err(|_| "Credential manager lock poisoned".to_string())?
            .store(service, data)
    }

    /// Retrieve and decrypt a credential.
    pub fn retrieve(&self, service: &str) -> Result<Option<Vec<u8>>, String> {
        self.vault
            .lock()
            .map_err(|_| "Credential manager lock poisoned".to_string())?
            .retrieve(service)
    }

    /// Remove a stored credential.
    pub fn delete(&self, service: &str) -> Result<(), String> {
        self.vault
            .lock()
            .map_err(|_| "Credential manager lock poisoned".to_string())?
            .delete(service)
    }

    /// List registered service names.
    pub fn list_services(&self) -> Result<Vec<String>, String> {
        self.vault
            .lock()
            .map_err(|_| "Credential manager lock poisoned".to_string())?
            .list_services()
    }

    /// Clear all stored credentials.
    pub fn clear(&self) -> Result<(), String> {
        self.vault
            .lock()
            .map_err(|_| "Credential manager lock poisoned".to_string())?
            .clear()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_vault() -> (Manager, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("credentials.db");
        let mgr = Manager::new(path).unwrap();
        (mgr, dir)
    }

    #[test]
    fn roundtrip_store_and_retrieve() {
        let (mgr, _dir) = temp_vault();
        let data = b"{\"access_token\":\"abc123\"}";
        mgr.store("spotify", data).unwrap();

        let result = mgr.retrieve("spotify").unwrap().unwrap();
        assert_eq!(result, data);
    }

    #[test]
    fn retrieve_missing_service_returns_none() {
        let (mgr, _dir) = temp_vault();
        let result = mgr.retrieve("spotify").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn delete_removes_credential() {
        let (mgr, _dir) = temp_vault();
        mgr.store("spotify", b"token").unwrap();
        mgr.delete("spotify").unwrap();
        assert!(mgr.retrieve("spotify").unwrap().is_none());
    }

    #[test]
    fn multiple_services_independent() {
        let (mgr, _dir) = temp_vault();
        mgr.store("spotify", b"spotify-token").unwrap();
        mgr.store("tidal", b"tidal-token").unwrap();

        assert_eq!(mgr.retrieve("spotify").unwrap().unwrap(), b"spotify-token");
        assert_eq!(mgr.retrieve("tidal").unwrap().unwrap(), b"tidal-token");

        let services = mgr.list_services().unwrap();
        assert_eq!(services, vec!["spotify", "tidal"]);
    }

    #[test]
    fn clear_removes_all() {
        let (mgr, _dir) = temp_vault();
        mgr.store("spotify", b"token").unwrap();
        mgr.store("tidal", b"token").unwrap();
        mgr.clear().unwrap();
        assert!(mgr.retrieve("spotify").unwrap().is_none());
        assert!(mgr.retrieve("tidal").unwrap().is_none());
        assert!(mgr.list_services().unwrap().is_empty());
    }

    #[test]
    fn vault_open_nonexistent_creates_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("new_vault.db");
        let vault = Vault::open(&path).unwrap();
        assert!(path.exists());
        vault.clear().unwrap();
    }

    #[test]
    fn update_existing_credential() {
        let (mgr, _dir) = temp_vault();
        mgr.store("spotify", b"token1").unwrap();
        mgr.store("spotify", b"token2").unwrap();
        assert_eq!(mgr.retrieve("spotify").unwrap().unwrap(), b"token2");
    }

    #[test]
    fn large_credential_payload() {
        let (mgr, _dir) = temp_vault();
        let large = vec![b'x'; 10_000];
        mgr.store("large", &large).unwrap();
        assert_eq!(mgr.retrieve("large").unwrap().unwrap(), large);
    }
}
