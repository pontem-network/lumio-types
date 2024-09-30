use rand::prelude::*;
use serde::{Deserialize, Serialize};

const CLAIM_EXPIRATION: u64 = 30;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Decoding JWT from hex failed ")]
    DecodeJwtHex(
        #[from]
        #[source]
        hex::FromHexError,
    ),
    #[error("Decoding JWT failed expected length {JWT_SECRET_LENGTH}, but got {0}")]
    DecodeJwtLength(usize),
    #[error("Decoding claim failed")]
    DecodeClaim(#[source] jsonwebtoken::errors::Error),
    #[error("Decoding claim failed")]
    EncodeClaim(#[source] jsonwebtoken::errors::Error),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Claims {
    exp: u64,
    iat: u64,
}

impl Claims {
    pub fn with_expiration(secs: u64) -> Self {
        let iat = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("Time went backwards")
            .as_secs();
        Self {
            iat,
            exp: iat + secs,
        }
    }
}

pub const JWT_SECRET_LENGTH: usize = 32;

#[derive(Debug, Clone, Copy)]
pub struct JwtSecret([u8; JWT_SECRET_LENGTH]);

impl std::fmt::Display for JwtSecret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", hex::encode(self.0))
    }
}

impl std::str::FromStr for JwtSecret {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self> {
        Self::from_hex(s)
    }
}

impl rand::distributions::Distribution<JwtSecret> for rand::distributions::Standard {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> JwtSecret {
        JwtSecret::new(rng.gen())
    }
}

impl JwtSecret {
    pub fn new(secret: [u8; JWT_SECRET_LENGTH]) -> Self {
        Self(secret)
    }

    pub fn from_hex(s: impl AsRef<[u8]>) -> Result<Self> {
        let vec = hex::decode(s)?;
        (&*vec)
            .try_into()
            .map(Self::new)
            .map_err(|_| Error::DecodeJwtLength(vec.len()))
    }

    pub fn decode(&self, token: impl AsRef<str>) -> Result<Claims> {
        jsonwebtoken::decode::<Claims>(
            token.as_ref(),
            &jsonwebtoken::DecodingKey::from_secret(&self.0),
            &jsonwebtoken::Validation::default(),
        )
        .map(|data| data.claims)
        .map_err(Error::DecodeClaim)
    }

    pub fn claim(&self) -> Result<String> {
        jsonwebtoken::encode(
            &Default::default(),
            // Expires in 30 secs from now
            &Claims::with_expiration(CLAIM_EXPIRATION),
            &jsonwebtoken::EncodingKey::from_secret(&self.0),
        )
        .map_err(Error::EncodeClaim)
    }
}
