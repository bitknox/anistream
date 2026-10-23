//! TLS 1.3 with raw public keys (RFC 7250) on both sides, on rustls and ring.
//!
//! No certificates and no authority: each side presents its Ed25519 key bare. The TLS layer only
//! proves that each side holds the key it presents. Whether that key is welcome is decided above
//! it: the TV looks the phone's key up among its paired devices, and the phone holds the TV to the
//! key it pinned when they paired.

use std::sync::{Arc, Mutex, PoisonError};

use rustls::{
    ClientConfig, DigitallySignedStruct, DistinguishedName, Error as TlsError, ServerConfig,
    SignatureScheme,
    client::{
        AlwaysResolvesClientRawPublicKeys,
        danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier},
    },
    crypto::{
        CryptoProvider, WebPkiSupportedAlgorithms, ring::default_provider,
        verify_tls13_signature_with_raw_key,
    },
    pki_types::{CertificateDer, ServerName, SubjectPublicKeyInfoDer, UnixTime},
    server::{
        AlwaysResolvesServerRawPublicKeys,
        danger::{ClientCertVerified, ClientCertVerifier},
    },
    version::TLS13,
};

use crate::{
    Error,
    identity::{Fingerprint, Identity},
};

/// What both sides name the protocol in the handshake. A TV that speaks a later major version
/// offers both, and the phone picks: see the design doc's versioning section.
pub const ALPN: &[u8] = b"anistream-remote/1";

/// The name a phone dials. Raw public keys carry no name, so it is checked by nobody; rustls only
/// needs one to start a handshake.
pub(crate) const SERVER_NAME: &str = "tv.anistream.invalid";

/// The label the pairing exchange binds itself to this connection with (RFC 8446, section 7.5).
pub(crate) const EXPORTER_LABEL: &[u8] = b"EXPORTER-anistream-remote-pairing";

fn provider() -> Arc<CryptoProvider> {
    Arc::new(default_provider())
}

fn algorithms() -> WebPkiSupportedAlgorithms {
    default_provider().signature_verification_algorithms
}

/// The TV's side: its own key, and any phone's key taken at the TLS layer.
pub(crate) fn server_config(identity: &Identity) -> Result<Arc<ServerConfig>, Error> {
    let mut config = ServerConfig::builder_with_provider(provider())
        .with_protocol_versions(&[&TLS13])?
        .with_client_cert_verifier(Arc::new(AnyClientKey { algorithms: algorithms() }))
        .with_cert_resolver(Arc::new(AlwaysResolvesServerRawPublicKeys::new(
            identity.certified()?,
        )));
    config.alpn_protocols = vec![ALPN.to_vec()];
    Ok(Arc::new(config))
}

/// The phone's side: its own key, and the TV's held to `pinned`, or taken on first use when it is
/// `None`. Taking it is safe only because pairing then checks it: the code the TV shows binds the
/// TV's key into the exchange, so a key that is not the TV's fails the pairing.
pub(crate) fn client_config(
    identity: &Identity,
    pinned: Option<Fingerprint>,
) -> Result<(Arc<ClientConfig>, Arc<ServerKey>), Error> {
    let server_key =
        Arc::new(ServerKey { pinned, seen: Mutex::new(None), algorithms: algorithms() });
    let mut config = ClientConfig::builder_with_provider(provider())
        .with_protocol_versions(&[&TLS13])?
        .dangerous()
        .with_custom_certificate_verifier(server_key.clone())
        .with_client_cert_resolver(Arc::new(AlwaysResolvesClientRawPublicKeys::new(
            identity.certified()?,
        )));
    config.alpn_protocols = vec![ALPN.to_vec()];
    Ok((Arc::new(config), server_key))
}

pub(crate) fn server_name() -> ServerName<'static> {
    ServerName::try_from(SERVER_NAME).expect("a valid DNS name")
}

/// The fingerprint of the key a peer presented, which the handshake proved it holds.
pub(crate) fn peer_fingerprint(certs: Option<&[CertificateDer<'_>]>) -> Option<Fingerprint> {
    certs?.first().map(|spki| Fingerprint::of_spki(spki.as_ref()))
}

/// Takes every client key at the TLS layer. The session decides what an unpaired key may do,
/// which is to pair and nothing else.
#[derive(Debug)]
struct AnyClientKey {
    algorithms: WebPkiSupportedAlgorithms,
}

impl ClientCertVerifier for AnyClientKey {
    fn root_hint_subjects(&self) -> &[DistinguishedName] {
        &[]
    }

    fn client_auth_mandatory(&self) -> bool {
        true
    }

    fn verify_client_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _now: UnixTime,
    ) -> Result<ClientCertVerified, TlsError> {
        Ok(ClientCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        Err(TlsError::PeerIncompatible(rustls::PeerIncompatible::Tls12NotOffered))
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        let spki = SubjectPublicKeyInfoDer::from(cert.as_ref());
        verify_tls13_signature_with_raw_key(message, &spki, dss, &self.algorithms)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        vec![SignatureScheme::ED25519]
    }

    fn requires_raw_public_keys(&self) -> bool {
        true
    }
}

/// The TV's key as the phone checks it: against the pin when there is one, and noted either way
/// for pairing to bind.
#[derive(Debug)]
pub(crate) struct ServerKey {
    pinned: Option<Fingerprint>,
    seen: Mutex<Option<Fingerprint>>,
    algorithms: WebPkiSupportedAlgorithms,
}

impl ServerKey {
    /// The key the TV presented on the last handshake.
    pub(crate) fn seen(&self) -> Option<Fingerprint> {
        *self.seen.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl ServerCertVerifier for ServerKey {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, TlsError> {
        let presented = Fingerprint::of_spki(end_entity.as_ref());
        if let Some(pinned) = self.pinned
            && pinned != presented
        {
            return Err(TlsError::General("the TV's key is not the one it paired with".into()));
        }
        *self.seen.lock().unwrap_or_else(PoisonError::into_inner) = Some(presented);
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        Err(TlsError::PeerIncompatible(rustls::PeerIncompatible::Tls12NotOffered))
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        let spki = SubjectPublicKeyInfoDer::from(cert.as_ref());
        verify_tls13_signature_with_raw_key(message, &spki, dss, &self.algorithms)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        vec![SignatureScheme::ED25519]
    }

    fn requires_raw_public_keys(&self) -> bool {
        true
    }
}
