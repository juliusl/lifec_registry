use std::path::PathBuf;

use specs::{Component, VecStorage};

/// Component for local content,
/// 
#[derive(Component)]
#[storage(VecStorage)]
pub struct Local {
    /// Path to local content,
    /// 
    pub path: PathBuf
}