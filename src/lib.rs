#![warn(unused_crate_dependencies)]

pub mod h256;
pub mod rpc;

pub type Address = h256::H256;
pub type Hash = h256::H256;

pub type Slot = u64;
pub type Version = u64;
pub type PayloadId = u64;

pub type UnixTimestamp = i64;
