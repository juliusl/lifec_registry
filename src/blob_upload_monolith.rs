use lifec::{plugins::{Plugin, ThunkContext}, DenseVecStorage, Component};

/// BlobImport handler based on OCI spec endpoints: 
/// 
/// ```markdown
/// | ID     | Method         | API Endpoint                                                 | Success     | Failure           |
/// | ------ | -------------- | ------------------------------------------------------------ | ----------- | ----------------- |
/// | end-4a | `POST`         | `/v2/<name>/blobs/uploads/`                                  | `202`       | `404`             |
/// | end-4b | `POST`         | `/v2/<name>/blobs/uploads/?digest=<digest>`                  | `201`/`202` | `404`/`400`       |
/// | end-11 | `POST`         | `/v2/<name>/blobs/uploads/?mount=<digest>&from=<other_name>` | `201`       | `404`             |
/// ```
/// 
#[derive(Component, Default)]
#[storage(DenseVecStorage)]
pub struct BlobUploadMonolith;

impl Plugin for BlobUploadMonolith {
    fn symbol() -> &'static str {
        "blob_upload_monolith"
    }

    fn call(_: &ThunkContext) -> Option<lifec::plugins::AsyncContext> {
        todo!()
    }
}