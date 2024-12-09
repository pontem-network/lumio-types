use std::{fmt::Debug, ops::Deref};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum To<T> {
    OpSol(T),
    OpMove(T),
    Lumio(T),
    Any(T),
}

impl<T> To<T> {
    pub fn into_inner(self) -> T {
        match self {
            To::OpSol(inner) => inner,
            To::OpMove(inner) => inner,
            To::Lumio(inner) => inner,
            To::Any(inner) => inner,
        }
    }

    pub fn inner(&self) -> &T {
        match self {
            To::OpSol(inner) => inner,
            To::OpMove(inner) => inner,
            To::Lumio(inner) => inner,
            To::Any(inner) => inner,
        }
    }

    pub fn for_lumio(self) -> Option<T> {
        match self {
            To::OpSol(_) | To::OpMove(_) => None,
            To::Lumio(inner) | To::Any(inner) => Some(inner),
        }
    }

    pub fn for_op_sol(self) -> Option<T> {
        match self {
            To::Lumio(_) | To::OpMove(_) => None,
            To::OpSol(inner) | To::Any(inner) => Some(inner),
        }
    }

    pub fn for_op_move(self) -> Option<T> {
        match self {
            To::Lumio(_) | To::OpSol(_) => None,
            To::OpMove(inner) | To::Any(inner) => Some(inner),
        }
    }
}

impl<T> Deref for To<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.inner()
    }
}

impl<T> AsRef<T> for To<T> {
    fn as_ref(&self) -> &T {
        self.inner()
    }
}
