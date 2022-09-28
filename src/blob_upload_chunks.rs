use lifec::{plugins::{Plugin, ThunkContext}, DenseVecStorage, Component};

/// BlobImport handler based on OCI spec endpoints: 
/// 
/// 
/// ```markdown
/// | ID     | Method         | API Endpoint                                                 | Success     | Failure           |
/// | ------ | -------------- | ------------------------------------------------------------ | ----------- | ----------------- |
/// | end-5  | `PATCH`        | `/v2/<name>/blobs/uploads/<reference>`                       | `202`       | `404`/`416`       |
/// | end-6  | `PUT`          | `/v2/<name>/blobs/uploads/<reference>?digest=<digest>`       | `201`       | `404`/`400`       |
/// ```
///
#[derive(Component, Default)]
#[storage(DenseVecStorage)]
pub struct BlobUploadChunks;

impl Plugin for BlobUploadChunks {
    fn symbol() -> &'static str {
        "blob_upload_chunks"
    }

    fn call(context: &ThunkContext) -> Option<lifec::plugins::AsyncContext> {
        todo!()
    }
}