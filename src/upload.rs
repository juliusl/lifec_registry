use lifec::{Plugin, BlockObject, BlockProperties};


/// Plugin to upload registry content
/// 
#[derive(Default)]
pub struct Upload;

impl Plugin for Upload {
    fn symbol() -> &'static str {
        "upload"
    }

    fn description() -> &'static str {
        "Uploads content to the registry"
    }

    fn call(context: &lifec::ThunkContext) -> Option<lifec::AsyncContext> {
        todo!()
    }
}

impl BlockObject for Upload {
    fn query(&self) -> BlockProperties {
        BlockProperties::default()
    }

    fn parser(&self) -> Option<lifec::CustomAttribute> {
        Some(Self::as_custom_attr())
    }
}

