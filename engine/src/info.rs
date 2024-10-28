use lumio_types::Hash;
use lumio_types::{Slot, Version};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct L2Info {
    /// Hash of l2 genesis block. May be used to identify the chain. l1 <-> l2 bridge may use it to verify the chain.
    pub genesis_hash: Hash,
    /// Confirmed l1 slot
    pub l1_confirmed_slot: Slot,
    /// L2 version. slot on solana or ledger version on aptos.
    pub l2_version: Version,
}
