pub mod events;
pub mod h256;
pub mod p2p;
pub mod to;

pub type Address = h256::H256;
pub type Hash = h256::H256;

pub type Slot = u64;
pub type Version = u64;

pub type UnixTimestamp = i64;

pub type Transaction = Vec<u8>;
