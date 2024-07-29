use std::num::NonZeroU32;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct PayloadBuilderConfig {
    pub max_slots_per_builder_tick: NonZeroU32,
    pub max_cache_size: NonZeroU32,
}

impl Default for PayloadBuilderConfig {
    fn default() -> Self {
        Self {
            max_slots_per_builder_tick: NonZeroU32::new(256).unwrap(),
            max_cache_size: NonZeroU32::new(1024).unwrap(),
        }
    }
}
