//! Keys, signatures, secrets and digests.
//!
//! Pure Rust — `ed25519-dalek` and `sha2` — so the same code signs an index in the reference
//! server and in a Worker. The only decisions here are the encodings (unpadded base64url for
//! anything that ends up in a link or a header, lowercase hex for digests) and the signing
//! context, which stops a store's key from being usable to sign anything other than an index.

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use ed25519_dalek::{Signer, Verifier};
use sha2::{Digest, Sha256};

/// Prepended to every signed index before signing and verifying.
pub const INDEX_CONTEXT: &[u8] = b"anistream-store-index-v1\n";

/// What a token starts with. Lets a secret scanner, or a person, recognise one in a paste.
pub const TOKEN_PREFIX: &str = "ast1_";

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CryptoError {
    #[error("not a store key")]
    BadKey,
    #[error("the signature does not match")]
    BadSignature,
    #[error("the system random number generator failed")]
    Random,
    #[error("the signing key file is unreadable")]
    BadSigningKey,
}

/// A store's Ed25519 public key.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct PublicKey([u8; 32]);

impl std::fmt::Debug for PublicKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "PublicKey({})", self.fingerprint())
    }
}

impl PublicKey {
    /// Read the unpadded base64url form used in invite links and the protocol.
    pub fn parse(text: &str) -> Result<Self, CryptoError> {
        let bytes = URL_SAFE_NO_PAD.decode(text.trim()).map_err(|_| CryptoError::BadKey)?;
        let bytes: [u8; 32] = bytes.try_into().map_err(|_| CryptoError::BadKey)?;
        Ok(Self(bytes))
    }

    pub fn encode(&self) -> String {
        URL_SAFE_NO_PAD.encode(self.0)
    }

    /// A short, stable name for the key: the first eight bytes of its SHA-256, in hex.
    ///
    /// Also what a client uses as the store's local id, so joining the same store twice is
    /// recognised as the same store whatever address it was reached at.
    pub fn id(&self) -> String {
        sha256_hex(&self.0)[..16].to_owned()
    }

    /// [`Self::id`] in groups of four, for showing to a person.
    pub fn fingerprint(&self) -> String {
        let id = self.id();
        id.as_bytes()
            .chunks(4)
            .map(|chunk| String::from_utf8_lossy(chunk).into_owned())
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Check an index signature. See [`SigningKey::sign_index`].
    pub fn verify_index(&self, index: &str, signature: &str) -> Result<(), CryptoError> {
        let signature =
            URL_SAFE_NO_PAD.decode(signature.trim()).map_err(|_| CryptoError::BadSignature)?;
        let signature = ed25519_dalek::Signature::from_slice(&signature)
            .map_err(|_| CryptoError::BadSignature)?;
        let key = ed25519_dalek::VerifyingKey::from_bytes(&self.0)
            .map_err(|_| CryptoError::BadKey)?;
        key.verify(&signed_message(index), &signature).map_err(|_| CryptoError::BadSignature)
    }
}

/// A store's Ed25519 key pair.
pub struct SigningKey(ed25519_dalek::SigningKey);

impl std::fmt::Debug for SigningKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SigningKey({})", self.public().fingerprint())
    }
}

impl SigningKey {
    pub fn generate() -> Result<Self, CryptoError> {
        Ok(Self(ed25519_dalek::SigningKey::from_bytes(&random::<32>()?)))
    }

    /// The form a key is kept in: its 32-byte seed, unpadded base64url. Everything else about the
    /// key is derived from the seed.
    pub fn encode(&self) -> String {
        URL_SAFE_NO_PAD.encode(self.0.to_bytes())
    }

    pub fn parse(text: &str) -> Result<Self, CryptoError> {
        let seed =
            URL_SAFE_NO_PAD.decode(text.trim()).map_err(|_| CryptoError::BadSigningKey)?;
        let seed: [u8; 32] = seed.try_into().map_err(|_| CryptoError::BadSigningKey)?;
        Ok(Self(ed25519_dalek::SigningKey::from_bytes(&seed)))
    }

    pub fn public(&self) -> PublicKey {
        PublicKey(self.0.verifying_key().to_bytes())
    }

    /// Sign an index document, returning the signature in unpadded base64url.
    ///
    /// What is signed is [`INDEX_CONTEXT`] followed by the document's bytes, never the bare
    /// document, so a signature made here cannot be replayed as one over anything else.
    pub fn sign_index(&self, index: &str) -> String {
        URL_SAFE_NO_PAD.encode(self.0.sign(&signed_message(index)).to_bytes())
    }
}

fn signed_message(index: &str) -> Vec<u8> {
    let mut message = Vec::with_capacity(INDEX_CONTEXT.len() + index.len());
    message.extend_from_slice(INDEX_CONTEXT);
    message.extend_from_slice(index.as_bytes());
    message
}

fn random<const N: usize>() -> Result<[u8; N], CryptoError> {
    let mut bytes = [0u8; N];
    getrandom::getrandom(&mut bytes).map_err(|_| CryptoError::Random)?;
    Ok(bytes)
}

/// A new bearer token: 32 random bytes behind [`TOKEN_PREFIX`].
pub fn new_token() -> Result<String, CryptoError> {
    Ok(format!("{TOKEN_PREFIX}{}", URL_SAFE_NO_PAD.encode(random::<32>()?)))
}

/// A new invite code: 16 random bytes, short enough to sit comfortably in a link.
pub fn new_code() -> Result<String, CryptoError> {
    Ok(URL_SAFE_NO_PAD.encode(random::<16>()?))
}

/// Whether `code` has the shape [`new_code`] produces. Checked before any lookup.
pub fn is_code(code: &str) -> bool {
    URL_SAFE_NO_PAD.decode(code).is_ok_and(|bytes| bytes.len() == 16)
}

/// A new record id: 8 random bytes in hex. Not a secret, only unique.
pub fn new_id() -> Result<String, CryptoError> {
    Ok(hex(&random::<8>()?))
}

/// The digest a server stores in place of a token or invite code.
///
/// A plain SHA-256 is enough: both are 128 or more random bits, so there is nothing for a slow
/// hash to protect, and a lookup by digest needs the digest to be deterministic.
pub fn hash_secret(secret: &str) -> String {
    sha256_hex(secret.trim().as_bytes())
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(DIGITS[usize::from(byte >> 4)] as char);
        out.push(DIGITS[usize::from(byte & 0x0f)] as char);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_signed_index_verifies_with_its_own_key() {
        let key = SigningKey::generate().unwrap();
        let index = r#"{"protocol":1,"plugins":[]}"#;
        let signature = key.sign_index(index);
        key.public().verify_index(index, &signature).unwrap();
    }

    #[test]
    fn a_changed_index_fails_verification() {
        let key = SigningKey::generate().unwrap();
        let signature = key.sign_index(r#"{"sequence":2}"#);
        assert_eq!(
            key.public().verify_index(r#"{"sequence":1}"#, &signature),
            Err(CryptoError::BadSignature)
        );
    }

    #[test]
    fn another_stores_key_fails_verification() {
        let ours = SigningKey::generate().unwrap();
        let theirs = SigningKey::generate().unwrap();
        let signature = theirs.sign_index("{}");
        assert_eq!(
            ours.public().verify_index("{}", &signature),
            Err(CryptoError::BadSignature)
        );
    }

    #[test]
    fn the_signature_covers_the_context_not_only_the_document() {
        // A raw Ed25519 signature over the bare document must not pass: the context is what
        // keeps a store key from signing anything else that happens to look like an index.
        let key = SigningKey::generate().unwrap();
        let bare = URL_SAFE_NO_PAD.encode(key.0.sign(b"{}").to_bytes());
        assert_eq!(key.public().verify_index("{}", &bare), Err(CryptoError::BadSignature));
    }

    #[test]
    fn a_key_survives_being_stored_and_read_back() {
        let key = SigningKey::generate().unwrap();
        let restored = SigningKey::parse(&key.encode()).unwrap();
        assert_eq!(key.public(), restored.public());
        let signature = restored.sign_index("{}");
        key.public().verify_index("{}", &signature).unwrap();
        assert!(SigningKey::parse("too short").is_err());
    }

    #[test]
    fn signatures_are_standard_ed25519() {
        // RFC 8032 test vector 1: the empty message under a known key. A store signed by the
        // reference server and checked by any other Ed25519 implementation must agree.
        let seed = "9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60";
        let seed: Vec<u8> =
            (0..32).map(|i| u8::from_str_radix(&seed[i * 2..i * 2 + 2], 16).unwrap()).collect();
        let key = ed25519_dalek::SigningKey::from_bytes(&seed.try_into().unwrap());
        assert_eq!(
            hex(&key.sign(b"").to_bytes()),
            "e5564300c360ac729086e2cc806e828a84877f1eb8e5d974d873e065224901555fb8821590a33bacc61e39701cf9b46bd25bf5f0595bbe24655141438e7a100b"
        );
    }

    #[test]
    fn a_public_key_round_trips_through_its_text_form() {
        let key = SigningKey::generate().unwrap().public();
        let text = key.encode();
        assert_eq!(text.len(), 43);
        assert_eq!(PublicKey::parse(&text).unwrap(), key);
    }

    #[test]
    fn a_truncated_or_padded_key_is_rejected() {
        let text = SigningKey::generate().unwrap().public().encode();
        assert_eq!(PublicKey::parse(&text[..40]), Err(CryptoError::BadKey));
        assert_eq!(PublicKey::parse(&format!("{text}AAAA")), Err(CryptoError::BadKey));
        assert_eq!(PublicKey::parse("not a key"), Err(CryptoError::BadKey));
    }

    #[test]
    fn the_fingerprint_is_the_id_in_groups() {
        let key = SigningKey::generate().unwrap().public();
        assert_eq!(key.id().len(), 16);
        assert_eq!(key.fingerprint().replace(' ', ""), key.id());
        assert_eq!(key.fingerprint().split(' ').count(), 4);
    }

    #[test]
    fn secrets_are_distinct_and_shaped_as_documented() {
        let a = new_token().unwrap();
        let b = new_token().unwrap();
        assert_ne!(a, b);
        assert!(a.starts_with(TOKEN_PREFIX));
        assert_eq!(a.len(), TOKEN_PREFIX.len() + 43);

        let code = new_code().unwrap();
        assert_eq!(code.len(), 22);
        assert!(is_code(&code));
        assert!(!is_code("short"));
        assert!(!is_code(&format!("{code}x")));

        assert_eq!(new_id().unwrap().len(), 16);
    }

    #[test]
    fn hashing_is_stable_and_ignores_surrounding_whitespace() {
        assert_eq!(hash_secret("abc"), hash_secret(" abc\n"));
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
