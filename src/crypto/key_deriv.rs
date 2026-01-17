//! Key derivation using Argon2id.

use crate::error::{AppError, Result};
use argon2::{Algorithm, Argon2, Params, Version};
use rand::RngCore;

/// Size of the salt in bytes.
pub const SALT_SIZE: usize = 16;

/// Size of the nonce in bytes (8 bytes for large file support).
pub const NONCE_SIZE: usize = 8;

/// Size of the derived key in bytes (AES-256).
pub const KEY_SIZE: usize = 32;

/// Argon2id parameters following OWASP recommendations.
const ARGON2_T_COST: u32 = 3;          // Iterations
const ARGON2_M_COST: u32 = 65536;      // Memory in KiB (64 MB)
const ARGON2_P_COST: u32 = 4;          // Parallelism

/// Generate a random salt for key derivation.
pub fn generate_salt() -> [u8; SALT_SIZE] {
    let mut salt = [0u8; SALT_SIZE];
    rand::thread_rng().fill_bytes(&mut salt);
    salt
}

/// Generate a random nonce for AES-CTR.
pub fn generate_nonce() -> [u8; NONCE_SIZE] {
    let mut nonce = [0u8; NONCE_SIZE];
    rand::thread_rng().fill_bytes(&mut nonce);
    nonce
}

/// Derive a 256-bit key from a password using Argon2id.
///
/// # Arguments
/// * `password` - The user's password.
/// * `salt` - A random 16-byte salt.
///
/// # Returns
/// A 32-byte key suitable for AES-256.
pub fn derive_key(password: &str, salt: &[u8; SALT_SIZE]) -> Result<[u8; KEY_SIZE]> {
    let params = Params::new(ARGON2_M_COST, ARGON2_T_COST, ARGON2_P_COST, Some(KEY_SIZE))
        .map_err(|e| AppError::Crypto(format!("Invalid Argon2 params: {}", e)))?;

    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

    let mut key = [0u8; KEY_SIZE];
    argon2
        .hash_password_into(password.as_bytes(), salt, &mut key)
        .map_err(|e| AppError::Crypto(format!("Key derivation failed: {}", e)))?;

    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kdf_consistency() {
        let password = "test_password_123";
        let salt = [0u8; SALT_SIZE]; // Fixed salt for testing

        let key1 = derive_key(password, &salt).unwrap();
        let key2 = derive_key(password, &salt).unwrap();

        assert_eq!(key1, key2, "Same password and salt should produce same key");
    }

    #[test]
    fn test_different_salts_produce_different_keys() {
        let password = "test_password";
        let salt1 = [0u8; SALT_SIZE];
        let salt2 = [1u8; SALT_SIZE];

        let key1 = derive_key(password, &salt1).unwrap();
        let key2 = derive_key(password, &salt2).unwrap();

        assert_ne!(key1, key2, "Different salts should produce different keys");
    }

    #[test]
    fn test_generate_salt_randomness() {
        let salt1 = generate_salt();
        let salt2 = generate_salt();

        assert_ne!(salt1, salt2, "Generated salts should be random");
    }

    #[test]
    fn test_generate_nonce_randomness() {
        let nonce1 = generate_nonce();
        let nonce2 = generate_nonce();

        assert_ne!(nonce1, nonce2, "Generated nonces should be random");
    }
}
