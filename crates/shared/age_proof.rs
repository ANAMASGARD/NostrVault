//! Bounded archive compatibility proof, not the production archive format.
use std::io::{Read, Write};

pub const MAX_PAYLOAD: usize = 1024 * 1024;
const MAX_CIPHERTEXT: usize = MAX_PAYLOAD + 65536;
pub const PROOF_PASSWORD: &str = "NostrVault public fixture password, never a user secret";

fn encrypt_with(encryptor: age::Encryptor, plaintext: &[u8]) -> Result<Vec<u8>, &'static str> {
    if plaintext.len() > MAX_PAYLOAD {
        return Err("limit");
    }
    let mut output = Vec::new();
    let mut writer = encryptor.wrap_output(&mut output).map_err(|_| "crypto")?;
    writer.write_all(plaintext).map_err(|_| "crypto")?;
    writer.finish().map_err(|_| "crypto")?;
    Ok(output)
}

pub fn encrypt_passphrase(plaintext: &[u8]) -> Result<Vec<u8>, &'static str> {
    if plaintext.len() > MAX_PAYLOAD {
        return Err("limit");
    }
    let mut recipient = age::scrypt::Recipient::new(PROOF_PASSWORD.into());
    recipient.set_work_factor(16);
    let encryptor =
        age::Encryptor::with_recipients(std::iter::once(&recipient as &dyn age::Recipient))
            .map_err(|_| "crypto")?;
    encrypt_with(encryptor, plaintext)
}

pub fn decrypt_passphrase(ciphertext: &[u8]) -> Result<Vec<u8>, &'static str> {
    if ciphertext.len() > MAX_CIPHERTEXT {
        return Err("limit");
    }
    let mut identity = age::scrypt::Identity::new(PROOF_PASSWORD.into());
    identity.set_max_work_factor(18);
    decrypt_with(ciphertext, &identity)
}

pub fn encrypt_recipient(plaintext: &[u8], recipient: &str) -> Result<Vec<u8>, &'static str> {
    let recipient: age::x25519::Recipient = recipient.parse().map_err(|_| "malformed")?;
    let encryptor =
        age::Encryptor::with_recipients(std::iter::once(&recipient as &dyn age::Recipient))
            .map_err(|_| "crypto")?;
    encrypt_with(encryptor, plaintext)
}

pub fn decrypt_recipient(ciphertext: &[u8], identity: &str) -> Result<Vec<u8>, &'static str> {
    let identity: age::x25519::Identity = identity.parse().map_err(|_| "malformed")?;
    decrypt_with(ciphertext, &identity)
}

fn decrypt_with(ciphertext: &[u8], identity: &dyn age::Identity) -> Result<Vec<u8>, &'static str> {
    if ciphertext.len() > MAX_CIPHERTEXT {
        return Err("limit");
    }
    let decryptor = age::Decryptor::new(ciphertext).map_err(|_| "authentication")?;
    let reader = decryptor
        .decrypt(std::iter::once(identity))
        .map_err(|error| match error {
            age::DecryptError::ExcessiveWork { .. } => "crypto_work",
            _ => "authentication",
        })?;
    let mut recovered = Vec::new();
    reader
        .take((MAX_PAYLOAD + 1) as u64)
        .read_to_end(&mut recovered)
        .map_err(|_| "authentication")?;
    if recovered.len() > MAX_PAYLOAD {
        return Err("limit");
    }
    Ok(recovered)
}

pub const REFERENCE_RECIPIENT: &str = include_str!("fixtures/age-recipient.txt");
pub const REFERENCE_IDENTITY: &str = include_str!("fixtures/age-identity.txt");
pub const REFERENCE_PASSPHRASE: &[u8] = include_bytes!("fixtures/reference-passphrase.age");
pub const REFERENCE_RECIPIENT_BYTES: &[u8] = include_bytes!("fixtures/reference-recipient.age");

/// Real adapter compatibility checks, also executed by the headless native proof.
pub fn verify_reference_proof(payload: &[u8]) -> Result<(), &'static str> {
    if decrypt_passphrase(REFERENCE_PASSPHRASE)? != payload
        || decrypt_recipient(REFERENCE_RECIPIENT_BYTES, REFERENCE_IDENTITY.trim())? != payload
    {
        return Err("crypto");
    }
    let ciphertext = encrypt_recipient(payload, REFERENCE_RECIPIENT.trim())?;
    if decrypt_recipient(&ciphertext, REFERENCE_IDENTITY.trim())? != payload {
        return Err("crypto");
    }
    let mut tampered = ciphertext.clone();
    let last = tampered.len().checked_sub(1).ok_or("crypto")?;
    tampered[last] ^= 1;
    if decrypt_recipient(&tampered, REFERENCE_IDENTITY.trim()).is_ok()
        || decrypt_recipient(&ciphertext[..last], REFERENCE_IDENTITY.trim()).is_ok()
    {
        return Err("crypto");
    }
    let mut excessive = REFERENCE_PASSPHRASE.to_vec();
    let marker = excessive
        .windows(4)
        .position(|bytes| bytes == b" 16\n")
        .ok_or("crypto")?;
    excessive[marker + 2] = b'9';
    if decrypt_passphrase(&excessive) != Err("crypto_work") {
        return Err("crypto");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn independent_references_and_authentication_failures() {
        let payload = vault_core::ZIP_BYTES;
        assert_eq!(decrypt_passphrase(REFERENCE_PASSPHRASE).unwrap(), payload);
        assert_eq!(
            decrypt_recipient(REFERENCE_RECIPIENT_BYTES, REFERENCE_IDENTITY.trim()).unwrap(),
            payload
        );
        let own = encrypt_recipient(payload, REFERENCE_RECIPIENT.trim()).unwrap();
        assert_eq!(
            decrypt_recipient(&own, REFERENCE_IDENTITY.trim()).unwrap(),
            payload
        );
        let mut wrong = age::scrypt::Identity::new("wrong public fixture password".into());
        wrong.set_max_work_factor(18);
        assert!(decrypt_with(REFERENCE_PASSPHRASE, &wrong).is_err());
        let own = encrypt_passphrase(payload).unwrap();
        assert_eq!(decrypt_passphrase(&own).unwrap(), payload);
        let mut changed = own.clone();
        let last = changed.len() - 1;
        changed[last] ^= 1;
        for bad in [&changed[..], &own[..own.len() - 1]] {
            assert!(decrypt_passphrase(bad).is_err());
        }
        let mut excessive = REFERENCE_PASSPHRASE.to_vec();
        let marker = excessive
            .windows(4)
            .position(|bytes| bytes == b" 16\n")
            .unwrap();
        excessive[marker + 2] = b'9';
        assert_eq!(decrypt_passphrase(&excessive), Err("crypto_work"));
    }
}
