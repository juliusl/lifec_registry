use hyper::Uri;
use specs::{Component, VecStorage};

/// Component for an upstream location
/// 
#[derive(Component)]
#[storage(VecStorage)]
pub struct Upstream {
    /// Location of the upstream content,
    /// 
    pub location: Uri
}