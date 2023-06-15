use specs::{SystemData, WriteStorage};
use specs::prelude::*;

use crate::{Descriptor, Platform};

use super::{Upstream, Local};

/// System data for content storage,
/// 
#[derive(SystemData)]
#[allow(dead_code)]
pub struct Contents<'a> {
    descriptors: WriteStorage<'a, Descriptor>,
    platforms: WriteStorage<'a, Platform>,
    upstreams: WriteStorage<'a, Upstream>,
    locals: WriteStorage<'a, Local>,
}

