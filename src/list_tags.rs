use lifec::{plugins::{Plugin, ThunkContext}, DenseVecStorage, Component};

/// BlobImport handler based on OCI spec endpoints: 
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


impl Plugin<ThunkContext> for ListTags {
    fn symbol() -> &'static str {
        "list_tags"
    }

    fn call_with_context(context: &mut ThunkContext) -> Option<lifec::plugins::AsyncContext> {
        todo!()
    }
}