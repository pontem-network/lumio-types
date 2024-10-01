use std::{
    fmt::{Debug, Display},
    str::FromStr,
};

use borsh::{BorshDeserialize, BorshSerialize};
use derive_more::{AsRef, From, Into};
use eyre::eyre;
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
    type Err = eyre::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let mut array = [0; 32];

        match (value, value.len()) {
            // Etherium | Aptos
            (s, l) if l > 44 || s.starts_with("0x") => {
                let value = s.trim_start_matches("0x");
                if value.len() == 64 {
                    hex::decode_to_slice(value, &mut array)?;
                } else {
                    hex::decode_to_slice(format!("{value:0>64}"), &mut array)?;
                }
            }
            // solana
            (s, _) => {
                array = bs58::decode(s)
                    .into_vec()?
                    .try_into()
                    .map_err(|_| eyre!("invalid address length"))?;
            }
        };

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
        let result = s.parse().map_err(serde::de::Error::custom);
        if result.is_err() {
            return Ok(H256::default());
        }
        result
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
        ] {
            let result = H256::from_str(address_str).unwrap();
            assert_eq!(
                address_str.trim_start_matches("0x"),
                result.to_string().trim_start_matches("0")
            );
        }
        let sol_address_str = "BFvf3yghkBJEZAbDzx6YpAcQdekm7frU3epKXdZmE7E5";
        let sol_address = H256::from_str(sol_address_str).unwrap();
        assert_eq!(&bs58::encode(sol_address.0).into_string(), sol_address_str);

        // https://raw.githubusercontent.com/pontem-network/eth-faucet-whitelist/main/src/mvm.whiteList.json
        let _: HashSet<H256> = serde_json::from_str(
            r#"["019b68599dd727829dfc5036dec02464abeacdf76e5d17ce43352533b1b212b8", "43417434fd869edee76cca2a4d2301e528a1551b1d719b75c350c3c97d15b8b9", "32d42921e177db242f5550ebd5a899fd84d539511d95f5f032e8a2a8900b6354", "a50a51ffd9db0009d3d75515e1ea414c5bd7f692e9c6d260d8643bda7e35b113"]"#,
        )
        .unwrap();

        // invalid value
        for s in [
            "a6a34cfb14dc99e5bd582fa0ac000d9e27c3437d7f63397ef384f147a44dd212c",
            "0xa6a34cfb14dc99e5bd582fa0ac000d9e27c3437d7f63397ef384f147a44dd212c",
        ] {
            assert!(H256::from_str(s).is_err());
        }

        // invalid value: a6a34cfb14dc99e5bd582fa0ac000d9e27c3437d7f63397ef384f147a44dd212c
        // Exists in the list https://raw.githubusercontent.com/pontem-network/eth-faucet-whitelist/main/src/mvm.whiteList.json
        let list: HashSet<H256> = serde_json::from_str(
            r#"[
                "9cf4723f4bd23a841e7a5bcdccc6e7fb35a76784e8c041a515f6368b17ebcbf",
                "019b68599dd727829dfc5036dec02464abeacdf76e5d17ce43352533b1b212b8",
                "a6a34cfb14dc99e5bd582fa0ac000d9e27c3437d7f63397ef384f147a44dd212c"
            ]"#,
        )
        .unwrap();
        assert!(list.contains(&H256::default()));

        // https://raw.githubusercontent.com/pontem-network/eth-faucet-whitelist/main/src/sol.whiteList.json
        let _: HashSet<H256> = serde_json::from_str(
            r#"["F7J6FsZivaRRyGpLWhTo3yc75R7Lid8xWH6we4LSqh4r", "o4MVa8H8Hnam9yb1D7TmHMjf1CDPGSikvkLQnghfzLo", "DvyGghtC2QL14u9W5QQ1fjWKW7uBQzWqnYTZZKXLJjN4"]"#,
        )
        .unwrap();
    }
}
