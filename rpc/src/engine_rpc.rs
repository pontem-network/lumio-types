use jsonrpsee::{proc_macros::rpc, types::ErrorObjectOwned};
use lumio_types::{
    payload::Payload,
    rpc::{AttributesArtifact, PayloadAttrs, PayloadStatus},
    Hash, PayloadId, Slot, Version,
};
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

#[rpc(server, client, namespace = "engine")]
pub trait L2EngineApi {
    #[method(name = "l2Info_v1")]
    async fn l2_info(&self) -> Result<L2Info, ErrorObjectOwned>;

    #[method(name = "versionInfo_v1")]
    async fn version_info(&self) -> Result<VersionInfo, ErrorObjectOwned>;

    #[method(name = "applyAttributes_v1")]
    async fn apply_attributes(
        &self,
        attrs: PayloadAttrs,
    ) -> Result<Option<AttributesArtifact>, ErrorObjectOwned>;

    #[method(name = "applyPayload_v1")]
    async fn apply_payload(&self, payload: Payload) -> Result<(), ErrorObjectOwned>;

    #[method(name = "setSyncMode_v1")]
    async fn set_sync_mode(&self, sync_mode: SyncMode) -> Result<(), ErrorObjectOwned>;

    #[method(name = "setPayloadStatus_v1")]
    async fn set_payload_status(
        &self,
        id: PayloadId,
        status: PayloadStatus,
    ) -> Result<(), ErrorObjectOwned>;
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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VersionInfo {
    pub version: String,
    pub git_commit: String,
}

#[cfg(test)]
mod test {
    use rand::random;

    use lumio_types::{
        payload::{Payload, SlotPayload},
        Hash,
    };

    #[test]
    fn test_serde_payload() {
        let tx = Payload {
            parent_payload: 1,
            slots: vec![SlotPayload {
                slot: 1,
                previous_blockhash: Hash::from(random::<[u8; 32]>()),
                blockhash: Hash::from(random::<[u8; 32]>()),
                block_time: None,
                block_height: None,
                txs: vec![vec![1, 2, 3]],
                bank_hash: Hash::from(random::<[u8; 32]>()),
            }],
            checkpoint: Hash::from(random::<[u8; 32]>()),
            id: 2,
        };
        let encoded_tx = serde_json::to_string(&tx).unwrap();
        let decoded_tx: Payload = serde_json::from_str(&encoded_tx).unwrap();
        assert_eq!(tx, decoded_tx);
    }
}
