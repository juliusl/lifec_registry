use lifec::{Component, HashMapStorage, Plugin, BlockObject, BlockProperties};

/// Streaming container image format,
/// 
#[derive(Component)]
#[storage(HashMapStorage)]
pub struct OverlayBD;

impl Plugin for OverlayBD {
    fn symbol() -> &'static str {
        "overlaybd"
    }

    fn call(context: &lifec::ThunkContext) -> Option<lifec::AsyncContext> {
        context.task(|_| {
            let tc = context.clone();
            async {
                // Status checks
                // 1) overlaybd is installed --
                // 2) 

                Some(tc)
            }
        })
    }
}

impl BlockObject for OverlayBD {
    fn query(&self) -> lifec::BlockProperties {
        BlockProperties::default()
    }

    fn parser(&self) -> Option<lifec::CustomAttribute> {
        Some(Self::as_custom_attr())
    }
}
