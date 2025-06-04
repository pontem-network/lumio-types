use borsh::{BorshDeserialize, BorshSerialize};
use derive_more::{AsRef, From, Into};
use eyre::eyre;
use serde::{Deserialize, Serialize};
use std::{
    fmt::{Debug, Display},
    str::FromStr,
};

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
pub struct EthAddress(pub [u8; 20]);

impl Display for EthAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", hex::encode(self.0))
    }
}

impl Debug for EthAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", hex::encode(self.0))
    }
}

impl EthAddress {
    pub fn inner(&self) -> [u8; 20] {
        self.0
    }
}

impl FromStr for EthAddress {
    type Err = eyre::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let mut array = [0; 20];
        let value = value.trim_start_matches("0x");
        if value.len() != 40 {
            return Err(eyre!("invalid address length: {}", value.len()));
        }
        hex::decode_to_slice(value, &mut array)
            .map_err(|_| eyre!("invalid address format: {}", value))?;
        Ok(EthAddress(array))
    }
}

impl Serialize for EthAddress {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for EthAddress {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s: &str = Deserialize::deserialize(deserializer)?;
        let result = s.parse().map_err(serde::de::Error::custom);
        if result.is_err() {
            return Ok(EthAddress::default());
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use rand::random;
    use super::*;

    #[test]
    fn test_address_serde() {
        let address = EthAddress::from(random::<[u8; 20]>());
        let json = serde_json::to_string(&address).unwrap();
        let de_addr = serde_json::from_str::<EthAddress>(&json).unwrap();
        assert_eq!(address, de_addr);
    }

    #[test]
    fn test_parse() {
        for address_str in [
            "0x863995c08b6e10ba2c2abb5985f3983e87e64f12",
            "863995c08b6e10ba2c2abb5985f3983e87e64f12",
        ] {
            let result = EthAddress::from_str(address_str).unwrap();
            assert_eq!(
                address_str.trim_start_matches("0x"),
                result.to_string().trim_start_matches("0")
            );
        }
    }
}
