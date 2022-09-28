use lifec::plugins::{Plugin, ThunkContext};

pub struct Index;

impl Plugin for Index {
    fn symbol() -> &'static str {
        "index"
    }

    fn call(context: &ThunkContext) -> Option<lifec::plugins::AsyncContext> {
        let tc = context.clone();
        context.task(|_| async {
            // TODO No-OP
            Some(tc)
        })
    }
}