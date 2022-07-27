use lifec::plugins::{Plugin, ThunkContext};

pub struct Index;

impl Plugin<ThunkContext> for Index {
    fn symbol() -> &'static str {
        todo!()
    }

    fn call_with_context(context: &mut ThunkContext) -> Option<lifec::plugins::AsyncContext> {
        todo!()
    }
}