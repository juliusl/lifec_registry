

mod mirror;
use lifec::{Runtime, editor::{Call, RuntimeEditor, Fix}, Extension, plugins::{Project, Expect, Missing}};
pub use mirror::Mirror;

mod blob_import;
pub use blob_import::BlobImport;

mod blob_upload_chunks;
pub use blob_upload_chunks::BlobUploadChunks;

mod blob_upload_monolith;
pub use blob_upload_monolith::BlobUploadMonolith;

mod blob_upload_session_id;
pub use blob_upload_session_id::BlobUploadSessionId;

mod download_blob;
pub use download_blob::DownloadBlob;

mod list_tags;
pub use list_tags::ListTags;

mod resolve;
pub use resolve::Resolve;

fn create_runtime(project: Project) -> Runtime {
    let mut runtime = Runtime::new(project);
    runtime.install::<Call, BlobImport>();
    runtime.install::<Call, BlobUploadChunks>();
    runtime.install::<Call, BlobUploadMonolith>();
    runtime.install::<Call, BlobUploadSessionId>();
    runtime.install::<Call, DownloadBlob>();
    runtime.install::<Call, ListTags>();
    runtime.install::<Call, Resolve>();
    runtime.install::<Call, Expect>();
    runtime.install::<Fix, Missing>();
    runtime
}

/// Represents the upstream registry that is being extended
#[derive(Default)]
pub struct Upstream {
    runtime_editor: RuntimeEditor,
    host_name: String,
}

impl From<RuntimeEditor> for Upstream {
    fn from(runtime_editor: RuntimeEditor) -> Self {
        Self { runtime_editor, host_name: String::default() }
    }
}

impl Extension for Upstream {

}

impl AsRef<Runtime> for Upstream {
    fn as_ref(&self) -> &Runtime {
        self.runtime_editor.runtime()
    }
}