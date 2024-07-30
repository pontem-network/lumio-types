use serde::{Deserialize, Serialize};
use strum::IntoStaticStr;

use super::Bridge;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, IntoStaticStr)]
pub enum L1Event {
    Deposit(Bridge),
}
