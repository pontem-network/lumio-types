use serde::{Deserialize, Serialize};

use crate::Address;

pub mod l1;
pub mod l2;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Transfer {
    pub account: Address,
    pub amount: u64,
}

#[cfg(test)]
mod test {
    use rand::random;

    use super::*;

    #[test]
    fn test_serde_tx_mint() {
        let tx = super::Transfer {
            account: Address::from(random::<[u8; 32]>()),
            amount: 100,
        };
        let encoded_tx = serde_json::to_string(&tx).unwrap();
        let decoded_tx: super::Transfer = serde_json::from_str(&encoded_tx).unwrap();
        assert_eq!(tx, decoded_tx);
    }
}
