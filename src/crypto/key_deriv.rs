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

/// Size of the MAC key in bytes (for authentication tag).
pub const MAC_KEY_SIZE: usize = 32;

/// Total size of derived key material (enc key + MAC key).
const DERIVED_KEY_SIZE: usize = KEY_SIZE + MAC_KEY_SIZE;

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
    Ok(derive_keys(password, salt)?.enc_key)
}

/// Derived keys for encryption and authentication.
#[derive(Debug, Clone, Copy)]
pub struct DerivedKeys {
    pub enc_key: [u8; KEY_SIZE],
    pub mac_key: [u8; MAC_KEY_SIZE],
}

/// Derive encryption and MAC keys from a password using Argon2id.
pub fn derive_keys(password: &str, salt: &[u8; SALT_SIZE]) -> Result<DerivedKeys> {
    let params = Params::new(ARGON2_M_COST, ARGON2_T_COST, ARGON2_P_COST, Some(DERIVED_KEY_SIZE))
        .map_err(|e| AppError::Crypto(format!("Invalid Argon2 params: {}", e)))?;

    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

    let mut out = [0u8; DERIVED_KEY_SIZE];
    argon2
        .hash_password_into(password.as_bytes(), salt, &mut out)
        .map_err(|e| AppError::Crypto(format!("Key derivation failed: {}", e)))?;

    let mut enc_key = [0u8; KEY_SIZE];
    enc_key.copy_from_slice(&out[0..KEY_SIZE]);

    let mut mac_key = [0u8; MAC_KEY_SIZE];
    mac_key.copy_from_slice(&out[KEY_SIZE..DERIVED_KEY_SIZE]);

    Ok(DerivedKeys { enc_key, mac_key })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kdf_consistency() {
        let password = "test_password_123";
        let salt = [0u8; SALT_SIZE]; // Fixed salt for testing

        let keys1 = derive_keys(password, &salt).unwrap();
        let keys2 = derive_keys(password, &salt).unwrap();

        assert_eq!(keys1.enc_key, keys2.enc_key, "Same password and salt should produce same enc key");
        assert_eq!(keys1.mac_key, keys2.mac_key, "Same password and salt should produce same mac key");
    }

    #[test]
    fn test_different_salts_produce_different_keys() {
        let password = "test_password";
        let salt1 = [0u8; SALT_SIZE];
        let salt2 = [1u8; SALT_SIZE];

        let keys1 = derive_keys(password, &salt1).unwrap();
        let keys2 = derive_keys(password, &salt2).unwrap();

        assert_ne!(keys1.enc_key, keys2.enc_key, "Different salts should produce different enc keys");
        assert_ne!(keys1.mac_key, keys2.mac_key, "Different salts should produce different mac keys");
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
