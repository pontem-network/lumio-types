use std::fmt::Debug;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum To<T> {
    OpSol(T),
    OpMove(T),
    Lumio(T),
}

impl<T> To<T> {
    pub fn into_inner(self) -> T {
        match self {
            To::OpSol(inner) => inner,
            To::OpMove(inner) => inner,
            To::Lumio(inner) => inner,
        }
    }
}
