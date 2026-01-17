//! Cryptographic engine module.
//!
//! Provides AES-256-CTR encryption/decryption and Argon2id key derivation.

mod engine;
mod key_deriv;

pub use engine::CryptoEngine;
pub use key_deriv::{derive_key, generate_nonce, generate_salt};
