use serde::{Deserialize, Serialize};

use crate::Address;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum L1Event {
    Deposit(TxMint),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TxMint {
    pub account: Address,
    pub amount: u64,
}

#[cfg(test)]
mod test {
    use rand::random;

    use super::*;

    #[test]
    fn test_serde_tx_mint() {
        let tx = super::TxMint {
            account: Address::from(random::<[u8; 32]>()),
            amount: 100,
        };
        let encoded_tx = serde_json::to_string(&tx).unwrap();
        let decoded_tx: super::TxMint = serde_json::from_str(&encoded_tx).unwrap();
        assert_eq!(tx, decoded_tx);
    }
}
