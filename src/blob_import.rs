use lifec::{plugins::{Plugin, ThunkContext}, DenseVecStorage, Component};


/// BlobImport handler based on OCI spec endpoints: 
/// 
/// ```markdown
/// | ID     | Method         | API Endpoint                                                 | Success     | Failure           |
/// | ------ | -------------- | ------------------------------------------------------------ | ----------- | ----------------- |
/// | end-11 | `POST`         | `/v2/<name>/blobs/uploads/?mount=<digest>&from=<other_name>` | `201`       | `404`             |
/// ```
/// 
#[derive(Component, Default)]
#[storage(DenseVecStorage)]
pub struct BlobImport;

impl Plugin for BlobImport {
    fn symbol() -> &'static str {
        "blob_import"
    }

    fn call(_: &ThunkContext) -> Option<lifec::plugins::AsyncContext> {
        todo!()
    }
}