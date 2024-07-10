use serde::{Deserialize, Serialize};

use super::Bridge;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum L2Event {
    Withdrawal(Bridge),
}
