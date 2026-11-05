//! VAPID (RFC 8292): the server's identity towards the push services. The ES256 JWT is signed
//! here with `p256`, so the dependency tree stays small (no `jwt-simple`, which pulls `rsa`).

use std::{fmt, str::FromStr};

use base64ct::{Base64UrlUnpadded, Encoding};
use chrono::{DateTime, TimeDelta, Utc};
use p256::{
    ecdsa::{Signature, SigningKey, signature::Signer},
    elliptic_curve::rand_core::OsRng,
};

/// How long a JWT is valid (push services accept at most 24 hours).
pub const JWT_VALIDITY: TimeDelta = TimeDelta::hours(1);

/// The server's P-256 key pair. `Debug` never shows the private key.
#[derive(Clone)]
pub struct VapidKey(SigningKey);

impl fmt::Debug for VapidKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("VapidKey(***)")
    }
}

impl PartialEq for VapidKey {
    fn eq(&self, other: &Self) -> bool {
        self.0.to_bytes() == other.0.to_bytes()
    }
}

impl Eq for VapidKey {}

/// Parses the private scalar: base64url without padding, 32 bytes.
impl FromStr for VapidKey {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let bytes = Base64UrlUnpadded::decode_vec(value.trim())
            .map_err(|_| "the key must be base64url without padding".to_owned())?;
        if bytes.len() != 32 {
            return Err(format!("the key must be 32 bytes, got {}", bytes.len()));
        }
        SigningKey::from_slice(&bytes)
            .map(Self)
            .map_err(|_| "the key is not a valid P-256 private key".to_owned())
    }
}

impl VapidKey {
    pub fn generate() -> Self {
        Self(SigningKey::random(&mut OsRng))
    }

    /// The private scalar, base64url without padding: the value of `VAPID_PRIVATE_KEY`.
    pub fn private_key_base64(&self) -> String {
        Base64UrlUnpadded::encode_string(&self.0.to_bytes())
    }

    /// The public key as an uncompressed SEC1 point (65 bytes), base64url without padding:
    /// what the browser needs as `applicationServerKey`.
    pub fn public_key_base64(&self) -> String {
        let point = self.0.verifying_key().to_encoded_point(false);
        Base64UrlUnpadded::encode_string(point.as_bytes())
    }

    /// The `Authorization` header value: `vapid t=<jwt>, k=<public key>`.
    ///
    /// The JWT audience is the origin of `endpoint`, it expires [`JWT_VALIDITY`] after `now`.
    pub fn authorization(
        &self,
        endpoint: &reqwest::Url,
        subject: &str,
        now: DateTime<Utc>,
    ) -> String {
        let header = r#"{"typ":"JWT","alg":"ES256"}"#;
        let claims = serde_json::json!({
            "aud": origin(endpoint),
            "exp": (now + JWT_VALIDITY).timestamp(),
            "sub": subject,
        });
        let signing_input = format!(
            "{}.{}",
            Base64UrlUnpadded::encode_string(header.as_bytes()),
            Base64UrlUnpadded::encode_string(claims.to_string().as_bytes()),
        );
        let signature: Signature = self.0.sign(signing_input.as_bytes());
        format!(
            "vapid t={signing_input}.{}, k={}",
            Base64UrlUnpadded::encode_string(&signature.to_bytes()),
            self.public_key_base64()
        )
    }
}

/// `scheme://host[:port]`, the port only when it is not the default of the scheme.
fn origin(url: &reqwest::Url) -> String {
    let host = url.host_str().unwrap_or_default();
    match url.port() {
        Some(port) => format!("{}://{host}:{port}", url.scheme()),
        None => format!("{}://{host}", url.scheme()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use p256::ecdsa::{VerifyingKey, signature::Verifier};

    #[test]
    fn round_trips_through_text() {
        let key = VapidKey::generate();
        let text = key.private_key_base64();
        assert_eq!(text.len(), 43);
        assert_eq!(text.parse::<VapidKey>().unwrap(), key);
        assert_eq!(
            text.parse::<VapidKey>().unwrap().public_key_base64(),
            key.public_key_base64()
        );
        // The public key is an uncompressed point: 65 bytes.
        let public = Base64UrlUnpadded::decode_vec(&key.public_key_base64()).unwrap();
        assert_eq!(public.len(), 65);
        assert_eq!(public[0], 4);
    }

    #[test]
    fn refuses_bad_keys() {
        let short = Base64UrlUnpadded::encode_string(&[1u8; 31]);
        let long = Base64UrlUnpadded::encode_string(&[1u8; 33]);
        let zero = Base64UrlUnpadded::encode_string(&[0u8; 32]);
        for bad in [
            short.as_str(),
            long.as_str(),
            zero.as_str(),
            "not base64!",
            "",
        ] {
            assert!(bad.parse::<VapidKey>().is_err(), "{bad:?}");
        }
        // Padding is not accepted: the key is base64url without it.
        let padded = format!("{}=", Base64UrlUnpadded::encode_string(&[1u8; 32]));
        assert!(padded.parse::<VapidKey>().is_err());
    }

    #[test]
    fn debug_hides_the_key() {
        let key = VapidKey::generate();
        let shown = format!("{key:?}");
        assert!(!shown.contains(&key.private_key_base64()), "{shown}");
        assert_eq!(shown, "VapidKey(***)");
    }

    fn decode_part(part: &str) -> serde_json::Value {
        serde_json::from_slice(&Base64UrlUnpadded::decode_vec(part).unwrap()).unwrap()
    }

    #[test]
    fn authorization_is_a_verifiable_vapid_header() {
        let key = VapidKey::generate();
        let now = Utc.timestamp_opt(1_800_000_000, 0).unwrap();
        let cases = [
            (
                "https://fcm.googleapis.com/fcm/send/abc?x=1",
                "https://fcm.googleapis.com",
            ),
            (
                "https://fcm.googleapis.com:443/x",
                "https://fcm.googleapis.com",
            ),
            ("http://127.0.0.1:8123/push", "http://127.0.0.1:8123"),
        ];
        for (endpoint, aud) in cases {
            let url: reqwest::Url = endpoint.parse().unwrap();
            let header = key.authorization(&url, "mailto:me@example.com", now);

            let rest = header.strip_prefix("vapid t=").expect("prefix");
            let (jwt, k) = rest.split_once(", k=").expect("t and k");
            assert_eq!(k, key.public_key_base64());

            let parts: Vec<&str> = jwt.split('.').collect();
            assert_eq!(parts.len(), 3);
            assert_eq!(
                decode_part(parts[0]),
                serde_json::json!({"typ": "JWT", "alg": "ES256"})
            );
            assert_eq!(
                decode_part(parts[1]),
                serde_json::json!({
                    "aud": aud,
                    "exp": 1_800_000_000 + 3600,
                    "sub": "mailto:me@example.com"
                })
            );

            let public = Base64UrlUnpadded::decode_vec(k).unwrap();
            let verifying = VerifyingKey::from_sec1_bytes(&public).unwrap();
            let raw = Base64UrlUnpadded::decode_vec(parts[2]).unwrap();
            assert_eq!(raw.len(), 64);
            let signature = Signature::from_slice(&raw).unwrap();
            let signing_input = format!("{}.{}", parts[0], parts[1]);
            verifying
                .verify(signing_input.as_bytes(), &signature)
                .expect("valid signature");
        }
    }
}
