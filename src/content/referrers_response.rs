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