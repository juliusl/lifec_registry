use lifec::{plugins::{Plugin, ThunkContext}, DenseVecStorage, Component};

/// ListTags  handler based on OCI spec endpoints: 
/// 
/// ```markdown
/// | ID     | Method         | API Endpoint                                                 | Success     | Failure           |
/// | ------ | -------------- | ------------------------------------------------------------ | ----------- | ----------------- |
/// | end-8a | `GET`          | `/v2/<name>/tags/list`                                       | `200`       | `404`             |
/// | end-8b | `GET`          | `/v2/<name>/tags/list?n=<integer>&last=<integer>`            | `200`       | `404`             |
/// ```
/// 
#[derive(Component, Default)]
#[storage(DenseVecStorage)]
pub struct ListTags;


impl Plugin for ListTags {
    fn symbol() -> &'static str {
        "list_tags"
    }

    fn call(context: &ThunkContext) -> Option<lifec::plugins::AsyncContext> {
        let tc = context.clone();
        context.task(|_| async {
            Some(tc)
        })
    }
}