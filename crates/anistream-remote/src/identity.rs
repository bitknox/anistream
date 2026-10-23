//! Who a TV or a phone is: an Ed25519 key of its own, made once and kept, and the fingerprint
//! the other side knows it by.

use std::{fmt, sync::Arc};

use ring::{
    digest,
    rand::SystemRandom,
    signature::{Ed25519KeyPair, KeyPair},
};
use rustls::{
    crypto::ring::sign::any_eddsa_type,
    pki_types::{CertificateDer, PrivatePkcs8KeyDer},
    sign::CertifiedKey,
};

use crate::Error;

/// The DER prefix of an Ed25519 `SubjectPublicKeyInfo`: the algorithm, then the 32 key bytes.
/// RFC 8410, section 4. This is the whole of what a raw public key (RFC 7250) carries.
const ED25519_SPKI_PREFIX: [u8; 12] =
    [0x30, 0x2a, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x03, 0x21, 0x00];

/// A device's own key pair, as PKCS#8. Made once per install and kept in the platform's secret
/// store (the engine's `Host::secrets`); a new one means every paired device pairs again.
#[derive(Clone)]
pub struct Identity {
    pkcs8: Arc<Vec<u8>>,
    public: [u8; 32],
}

impl Identity {
    /// A new key pair, from the system's random source.
    pub fn generate() -> Result<Self, Error> {
        let doc = Ed25519KeyPair::generate_pkcs8(&SystemRandom::new())
            .map_err(|_| Error::Crypto("no randomness for a new key"))?;
        Self::from_pkcs8(doc.as_ref())
    }

    /// The key pair kept from [`Self::to_pkcs8`].
    pub fn from_pkcs8(pkcs8: &[u8]) -> Result<Self, Error> {
        let pair = Ed25519KeyPair::from_pkcs8_maybe_unchecked(pkcs8)
            .map_err(|_| Error::Crypto("not an Ed25519 key"))?;
        let mut public = [0; 32];
        public.copy_from_slice(pair.public_key().as_ref());
        Ok(Self { pkcs8: Arc::new(pkcs8.to_vec()), public })
    }

    /// The bytes to keep, secret.
    pub fn to_pkcs8(&self) -> Vec<u8> {
        self.pkcs8.to_vec()
    }

    /// The public key as TLS presents it: a `SubjectPublicKeyInfo`, with no certificate around it.
    pub fn spki(&self) -> Vec<u8> {
        let mut spki = ED25519_SPKI_PREFIX.to_vec();
        spki.extend_from_slice(&self.public);
        spki
    }

    pub fn fingerprint(&self) -> Fingerprint {
        Fingerprint::of_spki(&self.spki())
    }

    /// The key as rustls signs a handshake with it.
    pub(crate) fn certified(&self) -> Result<Arc<CertifiedKey>, Error> {
        let key = any_eddsa_type(&PrivatePkcs8KeyDer::from(self.pkcs8.as_slice()))
            .map_err(|_| Error::Crypto("the key cannot sign"))?;
        Ok(Arc::new(CertifiedKey::new(vec![CertificateDer::from(self.spki())], key)))
    }
}

impl fmt::Debug for Identity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Identity")
            .field("fingerprint", &self.fingerprint())
            .finish_non_exhaustive()
    }
}

/// SHA-256 of a device's `SubjectPublicKeyInfo`: what a TV keeps of each paired phone, and what a
/// phone pins its TV by.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Fingerprint([u8; 32]);

impl Fingerprint {
    pub fn of_spki(spki: &[u8]) -> Self {
        let mut out = [0; 32];
        out.copy_from_slice(digest::digest(&digest::SHA256, spki).as_ref());
        Self(out)
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }

    pub fn from_hex(text: &str) -> Option<Self> {
        let bytes = hex::decode(text).ok()?;
        Some(Self(bytes.try_into().ok()?))
    }

    /// The first 16 hex digits: what the TXT record names a TV by, so a phone can tell a TV it
    /// knows when its address changed. Never a check on its own; the full fingerprint is.
    pub fn short(&self) -> String {
        hex::encode(&self.0[..8])
    }
}

impl fmt::Debug for Fingerprint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Fingerprint({})", self.short())
    }
}

impl fmt::Display for Fingerprint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_hex())
    }
}

impl serde::Serialize for Fingerprint {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_hex())
    }
}

impl<'de> serde::Deserialize<'de> for Fingerprint {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let text = String::deserialize(d)?;
        Self::from_hex(&text).ok_or_else(|| serde::de::Error::custom("not a fingerprint"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_kept_key_is_the_same_device() {
        let made = Identity::generate().unwrap();
        let kept = Identity::from_pkcs8(&made.to_pkcs8()).unwrap();
        assert_eq!(made.fingerprint(), kept.fingerprint());
        assert_ne!(made.fingerprint(), Identity::generate().unwrap().fingerprint());
    }

    #[test]
    fn the_spki_is_the_one_rustls_derives() {
        let id = Identity::generate().unwrap();
        let certified = id.certified().unwrap();
        let from_key = certified.key.public_key().unwrap();
        assert_eq!(from_key.as_ref(), id.spki().as_slice());
    }

    #[test]
    fn a_fingerprint_round_trips_as_hex() {
        let fp = Identity::generate().unwrap().fingerprint();
        assert_eq!(Fingerprint::from_hex(&fp.to_hex()), Some(fp));
        assert_eq!(fp.short().len(), 16);
    }
}
