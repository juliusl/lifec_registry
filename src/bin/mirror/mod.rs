use lifec::project::RunmdFile;

mod templates;
use templates::MIRROR_ENGINE_TEMPLATE;
use templates::MIRROR_TEMPLATE;

/// Returns a default root mirror file,
///
pub fn default_mirror_root() -> RunmdFile {
    RunmdFile {
        source: Some(MIRROR_TEMPLATE.to_string()),
        symbol: "".to_string(),
    }
}

/// Returns a default mirror engine file,
///
pub fn default_mirror_engine() -> RunmdFile {
    RunmdFile {
        symbol: "mirror".to_string(),
        source: Some(MIRROR_ENGINE_TEMPLATE.to_string()),
    }
}
