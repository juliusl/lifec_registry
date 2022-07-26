use lifec::{plugins::{Plugin, ThunkContext}, DenseVecStorage, Component};


/// BlobImport handler based on OCI spec endpoints: 
/// 
/// ```markdown
/// | ID     | Method         | API Endpoint                                                 | Success     | Failure           |
/// | ------ | -------------- | ------------------------------------------------------------ | ----------- | ----------------- |
/// | end-3  | `GET` / `HEAD` | `/v2/<name>/manifests/<reference>`                           | `200`       | `404`             |
/// | end-7  | `PUT`          | `/v2/<name>/manifests/<reference>`                           | `201`       | `404`             |
/// | end-9  | `DELETE`       | `/v2/<name>/manifests/<reference>`                           | `202`       | `404`/`400`/`405` |
/// ```
/// 
#[derive(Component, Default)]
#[storage(DenseVecStorage)]
pub struct Resolve;

impl Plugin<ThunkContext> for Resolve {
    fn symbol() -> &'static str {
        "resolve"
    }

    fn call_with_context(context: &mut ThunkContext) -> Option<lifec::plugins::AsyncContext> {
        todo!()
    }
}