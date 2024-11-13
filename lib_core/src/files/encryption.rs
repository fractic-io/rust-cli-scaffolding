use aes_gcm::aead::Aead;
use aes_gcm::{Aes256Gcm, Key, KeyInit as _, Nonce};
use argon2::Argon2;
use rand::RngCore;
use rand_core::OsRng;
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{Read as _, Write};

use crate::{define_cli_error, CliError};

define_cli_error!(
    FileEncryptionError,
    "File encryption error: {details}.",
    { details: &str }
);

fn derive_key(password: &str, salt: &[u8]) -> Result<[u8; 32], CliError> {
    let argon2 = Argon2::default();
    let mut key = [0u8; 32];
    argon2
        .hash_password_into(password.as_bytes(), salt, &mut key)
        .map_err(|e| FileEncryptionError::with_debug("failed to derive key", &e))?;
    Ok(key)
}

pub fn write_encrypted_file(filepath: &str, data: &str, password: &str) -> Result<(), CliError> {
    // Generate a random 16-byte salt.
    let mut salt = [0u8; 16];
    OsRng.fill_bytes(&mut salt);

    // Derive the key using the password and salt.
    let key_bytes = derive_key(password, &salt)?;
    let data_bytes = data.as_bytes();

    // Generate a random 12-byte nonce.
    let mut nonce_bytes = [0u8; 12];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    // Create an encryption key and cipher instance.
    let key = Key::<Aes256Gcm>::from_slice(&key_bytes);
    let cipher = Aes256Gcm::new(key);

    // Encrypt the data.
    let ciphertext = cipher
        .encrypt(nonce, data_bytes)
        .map_err(|e| FileEncryptionError::with_debug("failed to create ciphertext", &e))?;

    // Compute SHA256 hash of the plaintext.
    let mut hasher = Sha256::new();
    hasher.update(data_bytes);
    let hash = hasher.finalize(); // [u8; 32]

    // Write salt, nonce, hash, and ciphertext to file.
    let mut file = File::create(filepath)
        .map_err(|e| FileEncryptionError::with_debug("failed to create file", &e))?;
    file.write_all(&salt)
        .map_err(|e| FileEncryptionError::with_debug("failed to write salt", &e))?;
    file.write_all(&nonce_bytes)
        .map_err(|e| FileEncryptionError::with_debug("failed to write nonce", &e))?;
    file.write_all(&hash)
        .map_err(|e| FileEncryptionError::with_debug("failed to write hash", &e))?;
    file.write_all(&ciphertext)
        .map_err(|e| FileEncryptionError::with_debug("failed to write ciphertext", &e))?;

    Ok(())
}

pub fn read_encrypted_file(filepath: &str, password: &str) -> Result<String, CliError> {
    // Open and read the file.
    let mut file = File::open(filepath)
        .map_err(|e| FileEncryptionError::with_debug("failed to open file", &e))?;

    // Read the salt from the file.
    let mut salt = [0u8; 16];
    file.read_exact(&mut salt)
        .map_err(|e| FileEncryptionError::with_debug("failed to read salt", &e))?;

    // Read the nonce from the file.
    let mut nonce_bytes = [0u8; 12];
    file.read_exact(&mut nonce_bytes)
        .map_err(|e| FileEncryptionError::with_debug("failed to read nonce", &e))?;
    let nonce = Nonce::from_slice(&nonce_bytes);

    // Read the hash from the file.
    let mut hash_in_file = [0u8; 32];
    file.read_exact(&mut hash_in_file)
        .map_err(|e| FileEncryptionError::with_debug("failed to read hash", &e))?;

    // Read the ciphertext from the file.
    let mut ciphertext = Vec::new();
    file.read_to_end(&mut ciphertext)
        .map_err(|e| FileEncryptionError::with_debug("failed to read ciphertext", &e))?;

    // Derive the key using the password and salt.
    let key_bytes = derive_key(password, &salt)?;
    let key = Key::<Aes256Gcm>::from_slice(&key_bytes);
    let cipher = Aes256Gcm::new(key);

    // Decrypt the data.
    let plaintext = cipher
        .decrypt(nonce, &ciphertext[..])
        .map_err(|e| FileEncryptionError::with_debug("failed to decrypt ciphertext", &e))?;

    // Convert decrypted bytes back to a String.
    let plaintext_string = String::from_utf8(plaintext).map_err(|e| {
        FileEncryptionError::with_debug("failed to convert decrypted bytes to string", &e)
    })?;

    Ok(plaintext_string)
}

pub fn encrypted_file_matches_content(filepath: &str, content: &str) -> Result<bool, CliError> {
    // Open and read the file.
    let mut file = File::open(filepath)
        .map_err(|e| FileEncryptionError::with_debug("failed to open file", &e))?;

    // Read the salt and nonce (we don't need them here but need to skip them).
    let mut salt = [0u8; 16];
    file.read_exact(&mut salt)
        .map_err(|e| FileEncryptionError::with_debug("failed to read salt", &e))?;
    let mut nonce_bytes = [0u8; 12];
    file.read_exact(&mut nonce_bytes)
        .map_err(|e| FileEncryptionError::with_debug("failed to read nonce", &e))?;

    // Read the hash from the file.
    let mut hash_in_file = [0u8; 32];
    file.read_exact(&mut hash_in_file)
        .map_err(|e| FileEncryptionError::with_debug("failed to read hash", &e))?;

    // Compute the hash of the provided content.
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    let content_hash: [u8; 32] = hasher.finalize().into();

    // Compare the hashes.
    Ok(hash_in_file == content_hash)
}
