use std::{
    fmt::{Debug, Display},
    str::FromStr,
};

use anyhow::ensure;
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
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let bytes: Vec<u8> = match (value, value.len()) {
            // Etherium | Aptos
            (s, 64) => hex::decode(s)?,
            (s, _) if s.starts_with("0x") => {
                let mut value = s.trim_start_matches("0x").to_string();
                if value.len() < 64 {
                    value = format!("{value:0>64}")
                }
                hex::decode(&value)?
            }
            // solana
            (s, _) => bs58::decode(s).into_vec()?,
        };

        ensure!(
            bytes.len() == 32,
            "{}",
            hex::FromHexError::InvalidStringLength
        );

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
    use std::collections::HashSet;

    use rand::random;

    use super::*;

    #[test]
    fn test_address_serde() {
        let address = H256::from(random::<[u8; 32]>());
        let json = serde_json::to_string(&address).unwrap();
        let de_addr = serde_json::from_str::<H256>(&json).unwrap();
        assert_eq!(address, de_addr);
    }

    #[test]
    fn test_parse() {
        for address_str in [
            "0x863995c08b6e10ba2c2abb5985f3983e87e64f124099addffc5d3559d109c04e",
            "863995c08b6e10ba2c2abb5985f3983e87e64f124099addffc5d3559d109c04e",
            "0x1",
            "BFvf3yghkBJEZAbDzx6YpAcQdekm7frU3epKXdZmE7E5",
        ] {
            H256::from_str(address_str).unwrap();
        }

        // https://raw.githubusercontent.com/pontem-network/eth-faucet-whitelist/main/src/mvm.whiteList.json
        let _: HashSet<H256> = serde_json::from_str(
            r#"["019b68599dd727829dfc5036dec02464abeacdf76e5d17ce43352533b1b212b8", "43417434fd869edee76cca2a4d2301e528a1551b1d719b75c350c3c97d15b8b9", "32d42921e177db242f5550ebd5a899fd84d539511d95f5f032e8a2a8900b6354", "a50a51ffd9db0009d3d75515e1ea414c5bd7f692e9c6d260d8643bda7e35b113"]"#,
        )
        .unwrap();

        // https://raw.githubusercontent.com/pontem-network/eth-faucet-whitelist/main/src/sol.whiteList.json
        let _: HashSet<H256> = serde_json::from_str(
            r#"["F7J6FsZivaRRyGpLWhTo3yc75R7Lid8xWH6we4LSqh4r", "o4MVa8H8Hnam9yb1D7TmHMjf1CDPGSikvkLQnghfzLo", "DvyGghtC2QL14u9W5QQ1fjWKW7uBQzWqnYTZZKXLJjN4"]"#,
        )
        .unwrap();
    }
}
