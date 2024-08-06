use std::{
    fmt::{Debug, Display},
    str::FromStr,
};

use borsh::{BorshDeserialize, BorshSerialize};
use derive_more::{AsRef, From, Into};
use serde::{Deserialize, Serialize};

#[derive(
    Clone,
    Copy,
    PartialEq,
    BorshSerialize,
    BorshDeserialize,
    Default,
    From,
    Into,
    AsRef,
    Eq,
    PartialOrd,
    Ord,
    Hash,
)]
pub struct H256(pub [u8; 32]);

impl Display for H256 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", hex::encode(self.0))
    }
}

impl Debug for H256 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", hex::encode(self.0))
    }
}

impl H256 {
    pub fn inner(&self) -> [u8; 32] {
        self.0
    }
}

impl FromStr for H256 {
    type Err = hex::FromHexError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bytes = hex::decode(s)?;
        if bytes.len() != 32 {
            return Err(hex::FromHexError::InvalidStringLength);
        }
        let mut array = [0; 32];
        array.copy_from_slice(&bytes);
        Ok(H256(array))
    }
}

impl Serialize for H256 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for H256 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s: &str = Deserialize::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use rand::random;

    use super::*;

    #[test]
    fn test_address_serde() {
        let address = H256::from(random::<[u8; 32]>());
        let json = serde_json::to_string(&address).unwrap();
        let de_addr = serde_json::from_str::<H256>(&json).unwrap();
        assert_eq!(address, de_addr);
    }
}
