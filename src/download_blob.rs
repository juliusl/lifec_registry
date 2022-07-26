use lifec::{plugins::{Plugin, ThunkContext}, DenseVecStorage, Component};

/// BlobImport handler based on OCI spec endpoints: 
/// 
/// ```markdown
/// | ID     | Method         | API Endpoint                                                 | Success     | Failure           |
/// | ------ | -------------- | ------------------------------------------------------------ | ----------- | ----------------- |
/// | end-2  | `GET` / `HEAD` | `/v2/<name>/blobs/<digest>`                                  | `200`       | `404`             |
/// | end-10 | `DELETE`       | `/v2/<name>/blobs/<digest>`                                  | `202`       | `404`/`405`       |
/// ```
/// 
#[derive(Component, Default)]
#[storage(DenseVecStorage)]
pub struct DownloadBlob;

impl Plugin<ThunkContext> for DownloadBlob {
    fn symbol() -> &'static str {
        "download_blob"
    }

    fn call_with_context(context: &mut ThunkContext) -> Option<lifec::plugins::AsyncContext> {
        todo!()
    }
}