use std::fmt::Debug;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "eng")]
pub enum To<T> {
    OpSol(T),
    OpMove(T),
    Lumio(T),
}
