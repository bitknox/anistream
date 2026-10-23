//! Pairing: a six-digit code on the TV's screen, typed on the phone, turned into trust in each
//! other's key.
//!
//! The code is too short to be a key itself: anyone who saw an exchange keyed on six digits
//! alone could try all million offline. So it runs through SPAKE2, a password-authenticated key
//! exchange, which gives an attacker in the middle one guess per attempt and nothing to try
//! offline. Both sides' key fingerprints are SPAKE2's identities, and both proofs cover the TLS
//! connection's exported keying material, so the key reached belongs to these two keys on this
//! connection: a TV impostor with a key of its own, or a relay between two connections, ends up
//! with a different key and fails the proof. The TV allows [`MAX_TRIES`] wrong codes, then
//! drops the code, so a guess succeeds once in a few hundred thousand pairings.

use ring::{hmac, rand::SecureRandom, rand::SystemRandom};
use spake2::{Ed25519Group, Identity as SpakeId, Password, Spake2};

use crate::identity::Fingerprint;

/// Digits in a pairing code.
pub const CODE_DIGITS: u8 = 6;

/// Wrong codes a TV takes for one code before it drops it.
pub const MAX_TRIES: u32 = 3;

/// How long a code stays on the TV's screen, in seconds.
pub const CODE_LIFETIME: u32 = 120;

const TV_PROOF: &[u8] = b"anistream-remote pairing tv";
const PHONE_PROOF: &[u8] = b"anistream-remote pairing phone";

/// A code for the TV to show.
#[derive(Clone, PartialEq, Eq)]
pub struct PairingCode(String);

impl PairingCode {
    /// Six digits, each as likely as the next.
    pub fn generate() -> Self {
        let rng = SystemRandom::new();
        loop {
            let mut bytes = [0; 4];
            rng.fill(&mut bytes).expect("the system's random source");
            let n = u32::from_be_bytes(bytes);
            // The largest multiple of a million below 2^32, so every code is as likely.
            if n < 4_294_000_000 {
                return Self(format!("{:06}", n % 1_000_000));
            }
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    #[cfg(test)]
    pub(crate) fn fixed(code: &str) -> Self {
        Self(code.to_owned())
    }
}

impl std::fmt::Debug for PairingCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PairingCode(..)")
    }
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum PairError {
    #[error("the pairing message was malformed")]
    Malformed,
    #[error("the code was wrong, or someone is between the phone and the TV")]
    Mismatch,
}

/// Who is pairing, on which connection.
#[derive(Debug, Clone, Copy)]
pub struct Binding {
    pub phone: Fingerprint,
    pub tv: Fingerprint,
    /// The connection's exported keying material, [`crate::tls::EXPORTER_LABEL`].
    pub exporter: [u8; 32],
}

impl Binding {
    fn ids(&self) -> (SpakeId, SpakeId) {
        (SpakeId::new(self.phone.as_bytes()), SpakeId::new(self.tv.as_bytes()))
    }

    fn proof(&self, key: &[u8], label: &[u8]) -> Vec<u8> {
        let key = hmac::Key::new(hmac::HMAC_SHA256, key);
        let mut ctx = hmac::Context::with_key(&key);
        ctx.update(label);
        ctx.update(&self.exporter);
        ctx.sign().as_ref().to_vec()
    }

    fn check(&self, key: &[u8], label: &[u8], mac: &[u8]) -> Result<(), PairError> {
        let key = hmac::Key::new(hmac::HMAC_SHA256, key);
        let mut message = label.to_vec();
        message.extend_from_slice(&self.exporter);
        hmac::verify(&key, &message, mac).map_err(|_| PairError::Mismatch)
    }
}

/// The phone's side, between sending its message and reading the TV's.
pub struct PhonePairing {
    spake: Spake2<Ed25519Group>,
    binding: Binding,
}

impl PhonePairing {
    /// Start from the code the viewer typed: the message to send.
    pub fn start(code: &str, binding: Binding) -> (Self, Vec<u8>) {
        let (a, b) = binding.ids();
        let (spake, message) =
            Spake2::<Ed25519Group>::start_a(&Password::new(code.trim()), &a, &b);
        (Self { spake, binding }, message)
    }

    /// The TV's reply checked: the phone's proof to send back. A mismatch means the code was
    /// wrong or the TV is not the TV; the phone keeps no pin either way.
    pub fn finish(self, tv_message: &[u8], tv_mac: &[u8]) -> Result<Vec<u8>, PairError> {
        let key = self.spake.finish(tv_message).map_err(|_| PairError::Malformed)?;
        self.binding.check(&key, TV_PROOF, tv_mac)?;
        Ok(self.binding.proof(&key, PHONE_PROOF))
    }
}

/// The TV's side, between replying and reading the phone's proof.
pub struct TvPairing {
    key: Vec<u8>,
    binding: Binding,
}

impl TvPairing {
    /// Answer the phone's message with the code on screen: the TV's message and proof.
    pub fn respond(
        code: &PairingCode,
        binding: Binding,
        phone_message: &[u8],
    ) -> Result<(Self, Vec<u8>, Vec<u8>), PairError> {
        let (a, b) = binding.ids();
        let (spake, message) =
            Spake2::<Ed25519Group>::start_b(&Password::new(code.as_str()), &a, &b);
        let key = spake.finish(phone_message).map_err(|_| PairError::Malformed)?;
        let mac = binding.proof(&key, TV_PROOF);
        Ok((Self { key, binding }, message, mac))
    }

    /// Whether the phone reached the same key: the code was right, and this is the phone.
    pub fn confirm(&self, phone_mac: &[u8]) -> Result<(), PairError> {
        self.binding.check(&self.key, PHONE_PROOF, phone_mac)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::Identity;

    fn binding() -> Binding {
        Binding {
            phone: Identity::generate().unwrap().fingerprint(),
            tv: Identity::generate().unwrap().fingerprint(),
            exporter: [7; 32],
        }
    }

    fn run(
        tv_code: &str,
        phone_code: &str,
        tv: Binding,
        phone: Binding,
    ) -> Result<(), PairError> {
        let (phone_side, message) = PhonePairing::start(phone_code, phone);
        let (tv_side, reply, mac) =
            TvPairing::respond(&PairingCode::fixed(tv_code), tv, &message)?;
        let proof = phone_side.finish(&reply, &mac)?;
        tv_side.confirm(&proof)
    }

    #[test]
    fn the_right_code_pairs() {
        let b = binding();
        assert_eq!(run("123456", "123456", b, b), Ok(()));
    }

    #[test]
    fn a_wrong_code_fails_on_the_phone() {
        let b = binding();
        assert_eq!(run("123456", "123457", b, b), Err(PairError::Mismatch));
    }

    #[test]
    fn a_relay_between_two_connections_fails() {
        let phone = binding();
        let tv = Binding { exporter: [8; 32], ..phone };
        assert_eq!(run("123456", "123456", tv, phone), Err(PairError::Mismatch));
    }

    #[test]
    fn a_tv_impostor_with_its_own_key_fails_even_with_the_code() {
        let phone = binding();
        let tv = Binding { tv: Identity::generate().unwrap().fingerprint(), ..phone };
        assert_eq!(run("123456", "123456", tv, phone), Err(PairError::Mismatch));
    }

    #[test]
    fn codes_are_six_digits() {
        for _ in 0..50 {
            let code = PairingCode::generate();
            assert_eq!(code.as_str().len(), CODE_DIGITS as usize);
            assert!(code.as_str().bytes().all(|b| b.is_ascii_digit()));
        }
    }
}
