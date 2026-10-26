//! Sealing item bodies with XChaCha20-Poly1305.

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use zeroize::Zeroizing;

use crate::StoreError;
use crate::keys::KEY_LEN;

/// Length of the random nonce stored in front of each sealed body.
const NONCE_LEN: usize = 24;

/// Encrypts `plaintext` bound to `aad`; returns `nonce ‖ ciphertext`.
pub(crate) fn seal(
    key: &[u8; KEY_LEN],
    aad: &[u8],
    plaintext: &[u8],
) -> Result<Vec<u8>, StoreError> {
    let cipher = XChaCha20Poly1305::new_from_slice(key).map_err(|_| StoreError::Seal)?;
    let mut nonce = [0u8; NONCE_LEN];
    getrandom::fill(&mut nonce).map_err(|_| StoreError::Random)?;
    let ciphertext = cipher
        .encrypt(
            XNonce::from_slice(&nonce),
            Payload {
                msg: plaintext,
                aad,
            },
        )
        .map_err(|_| StoreError::Seal)?;
    Ok([nonce.as_slice(), &ciphertext].concat())
}

/// Decrypts what [`seal`] produced; fails on a wrong key, wrong `aad` or any change.
pub(crate) fn open(
    key: &[u8; KEY_LEN],
    aad: &[u8],
    sealed: &[u8],
) -> Result<Zeroizing<Vec<u8>>, StoreError> {
    if sealed.len() < NONCE_LEN {
        return Err(StoreError::Unseal);
    }
    let (nonce, ciphertext) = sealed.split_at(NONCE_LEN);
    let cipher = XChaCha20Poly1305::new_from_slice(key).map_err(|_| StoreError::Unseal)?;
    cipher
        .decrypt(
            XNonce::from_slice(nonce),
            Payload {
                msg: ciphertext,
                aad,
            },
        )
        .map(Zeroizing::new)
        .map_err(|_| StoreError::Unseal)
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: [u8; KEY_LEN] = [7; KEY_LEN];

    #[test]
    fn round_trip() {
        let sealed = seal(&KEY, b"aad", b"Hello from Supercy").unwrap();

        assert_eq!(
            open(&KEY, b"aad", &sealed).unwrap().as_slice(),
            b"Hello from Supercy"
        );
    }

    #[test]
    fn same_plaintext_seals_differently() {
        assert_ne!(
            seal(&KEY, b"", b"demo").unwrap(),
            seal(&KEY, b"", b"demo").unwrap()
        );
    }

    #[test]
    fn wrong_key_aad_or_any_flipped_bit_fails() {
        let sealed = seal(&KEY, b"aad", b"demo").unwrap();

        assert!(open(&[8; KEY_LEN], b"aad", &sealed).is_err());
        assert!(open(&KEY, b"other", &sealed).is_err());
        assert!(open(&KEY, b"aad", &sealed[..10]).is_err());
        for at in 0..sealed.len() {
            let mut tampered = sealed.clone();
            tampered[at] ^= 1;
            assert!(open(&KEY, b"aad", &tampered).is_err(), "bit flip at {at}");
        }
    }
}
