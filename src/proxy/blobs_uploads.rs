use super::proxy_route::RouteParameters;

/// Route plugin to handle registry blob uploads,
///
/// Example:
/// : .mirror     <azurecr.io>
/// : .host       <address> resolve, push
///
/// + .proxy      <address>
/// : .blobs_uploads
/// : .post        <operation-name>
///
#[derive(Default, Clone)]
pub struct BlobsUploads;

impl RouteParameters for BlobsUploads {
    fn path() -> &'static str {
        "/:repo<[a-zA-Z0-9/_-]+(?:blobs/uploads)>/"
    }

    fn ident() -> &'static str {
        "blobs_uploads"
    }
}