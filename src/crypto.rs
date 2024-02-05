use p384::ecdsa::SigningKey;
use rand_core::OsRng;

pub struct AuthKey {
    pub signing_key: SigningKey,
    pub public_key_b64: String,
}

impl AuthKey {
    pub fn new() -> Self {
        let signing_key = SigningKey::random(&mut OsRng);
        let public_key_der = signing_key.verifying_key().to_public_key_der().unwrap();
        Self {
            signing_key,
            public_key_b64: b64_std(public_key_der.as_bytes()),
        }
    }
}
