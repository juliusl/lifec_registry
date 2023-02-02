use lifec::prelude::{Component, DefaultVecStorage};
use serde::{Deserialize, Serialize};

use super::Descriptor;

/// Format of the response from the "referrers" api,
///
#[derive(Component, Default, Debug, Deserialize, Serialize)]
#[storage(DefaultVecStorage)]
pub struct ReferrersList {
    /// List of descriptors pointing to artifact manifests,
    ///
    pub referrers: Vec<Descriptor>,
}

impl ReferrersList {
    /// Finds all streamable descriptors from referrers,
    /// 
    /// Note: Currently there should only ever be one descriptor
    /// 
    pub fn find_streamable_descriptors(&self) -> Vec<Descriptor> {
        self.referrers
            .iter()
            .filter_map(|r| r.try_parse_streamable_descriptor())
            .collect()
    }
}
