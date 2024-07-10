use serde::{Deserialize, Serialize};

use super::Transfer;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum L2Event {
    Withdrawal(Transfer),
}
