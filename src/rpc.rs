use crate::{Address, Hash, PayloadId, Slot, UnixTimestamp, Version};
use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};
use sha3::Digest;
use std::fmt::Debug;
use jsonrpsee::{proc_macros::rpc, types::ErrorObjectOwned};

#[rpc(server, client, namespace = "engine")]
pub trait L2EngineApi<TX>
where
    TX: borsh::BorshSerialize + borsh::BorshDeserialize + std::fmt::Debug + PartialEq + Clone,
{
    #[method(name = "l2Info_v1")]
    async fn l2_info(&self) -> Result<L2Info, ErrorObjectOwned>;

    #[method(name = "applyAttributes_v1")]
    async fn apply_attributes(
        &self,
        attrs: PayloadAttrs,
    ) -> Result<Option<Payload<TX>>, ErrorObjectOwned>;

    #[method(name = "applyPayload_v1")]
    async fn apply_payload(&self, payload: Payload<TX>) -> Result<(), ErrorObjectOwned>;

    #[method(name = "setSyncMode_v1")]
    async fn set_sync_mode(&self, sync_mode: SyncMode) -> Result<(), ErrorObjectOwned>;
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TxMint {
    pub account: Address,
    pub amount: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PayloadAttrs {
    pub parent_payload: PayloadId,
    pub l1_slots: Vec<L1Slot>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct L1Slot {
    pub slot: Slot,
    pub deposits: Vec<TxMint>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct Payload<T>
where
    T: BorshSerialize + BorshDeserialize + Clone + Debug + PartialEq,
{
    pub id: PayloadId,
    pub parent_payload: PayloadId,
    pub slots: Vec<SlotPayload<T>>,
    pub checkpoint: Hash,
}

impl<T> Payload<T>
where
    T: BorshSerialize + BorshDeserialize + Clone + Debug + PartialEq,
{
    pub fn hash(&self) -> Hash {
        let serialized = borsh::to_vec(self).expect("Never fails");
        let digest = sha3::Sha3_256::digest(serialized);

        Hash::from(<[u8; 32]>::try_from(&digest[..]).expect("Never fails"))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct SlotPayload<T>
where
    T: BorshSerialize + BorshDeserialize + Clone + Debug + PartialEq,
{
    pub slot: Slot,
    pub previous_blockhash: Hash,
    pub blockhash: Hash,
    pub block_time: Option<UnixTimestamp>,
    pub block_height: Option<u64>,
    pub txs: Vec<T>,
    pub bank_hash: Hash,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct L2Info {
    /// Hash of l2 genesis block. May be used to identify the chain. l1 <-> l2 bridge may use it to verify the chain.
    pub genesis_hash: Hash,
    /// Submitted l1 slot
    pub l1_submitted_slot: Slot,
    /// Confirmed l1 slot
    pub l1_confirmed_slot: Slot,
    /// L2 version. slot on solana or ledger version on aptos.
    pub l2_version: Version,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default)]
pub enum SyncMode {
    Normal,
    #[default]
    Sync,
}

#[cfg(test)]
mod test {
    use rand::random;

    use crate::{rpc::Payload, Address, Hash};

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

    #[test]
    fn test_serde_payload() {
        let tx = Payload {
            parent_payload: 1,
            slots: vec![super::SlotPayload {
                slot: 1,
                previous_blockhash: Hash::from(random::<[u8; 32]>()),
                blockhash: Hash::from(random::<[u8; 32]>()),
                block_time: None,
                block_height: None,
                txs: vec![10],
                bank_hash: Hash::from(random::<[u8; 32]>()),
            }],
            checkpoint: Hash::from(random::<[u8; 32]>()),
            id: 2,
        };
        let encoded_tx = serde_json::to_string(&tx).unwrap();
        let decoded_tx: Payload<i32> = serde_json::from_str(&encoded_tx).unwrap();
        assert_eq!(tx, decoded_tx);
    }
}
