use lifec::{plugins::{Plugin, ThunkContext}, DenseVecStorage, Component};


/// Retrieves a blob upload session id from the registry
/// 
/// 
/// ``` markdown
/// | ID     | Method         | API Endpoint                                                 | Success     | Failure           |
/// | ------ | -------------- | ------------------------------------------------------------ | ----------- | ----------------- |
/// | end-4a | `POST`         | `/v2/<name>/blobs/uploads/`                                  | `202`       | `404`             |
/// ```
/// 
#[derive(Component, Default)]
#[storage(DenseVecStorage)]
pub struct BlobUploadSessionId; 

impl Plugin<ThunkContext> for BlobUploadSessionId {
    fn symbol() -> &'static str {
        "blob_upload_session_id"
    }

    fn call_with_context(context: &mut ThunkContext) -> Option<lifec::plugins::AsyncContext> {
        todo!()
    }
}