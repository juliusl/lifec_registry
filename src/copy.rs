use lifec::{BlockObject, BlockProperties, Plugin};

use crate::proxy::ProxyTarget;

/// Plugin to copy a manifest from one repo to another,
///
#[derive(Default)]
pub struct Copy;

impl Plugin for Copy {
    fn symbol() -> &'static str {
        "copy"
    }

    fn call(context: &lifec::ThunkContext) -> Option<lifec::AsyncContext> {
        context.task(|_| {
            let mut tc = context.clone();
            async {
                if let Some(proxy_target) = ProxyTarget::try_from(&tc).ok() {
                    
                }

                tc.copy_previous();
                Some(tc)
            }
        })
    }
}

impl BlockObject for Copy {
    fn query(&self) -> lifec::BlockProperties {
        BlockProperties::default()
            .require("copy")
    }

    fn parser(&self) -> Option<lifec::CustomAttribute> {
        Some(Self::as_custom_attr())
    }
}
