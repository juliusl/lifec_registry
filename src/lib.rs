

mod mirror;
use lifec::{Runtime, editor::{Call, RuntimeEditor, Fix}, Extension, plugins::{Project, Expect, Missing, Config}};
use lifec_poem::AppHost;
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

pub fn create_runtime(project: Project) -> Runtime {
    let mut runtime = Runtime::new(project);
    runtime.install::<Call, BlobImport>();
    runtime.install::<Call, BlobUploadChunks>();
    runtime.install::<Call, BlobUploadMonolith>();
    runtime.install::<Call, BlobUploadSessionId>();
    runtime.install::<Call, DownloadBlob>();
    runtime.install::<Call, ListTags>();
    runtime.install::<Call, Resolve>();
    runtime.install::<Call, Expect>();
    runtime.install::<Call, Mirror>();
    runtime.install::<Call, AppHost<Mirror>>();
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
    fn configure_app_world(world: &mut lifec::World) {
        RuntimeEditor::configure_app_world(world);
    }

    fn configure_app_systems(dispatcher: &mut lifec::DispatcherBuilder) {
        RuntimeEditor::configure_app_systems(dispatcher);
    }

    fn on_ui(&'_ mut self, app_world: &lifec::World, ui: &'_ imgui::Ui<'_>) {
        if ui.button("test") {
            self.runtime_editor.runtime_mut().add_config(Config("test", |a| {
                a.block.block_name = a.label("test").as_ref().to_string();

                a.as_mut()
                    .with_text("project_src", "examples/.runmd")
                    .with_text("address", "localhost:5000")
                    .with_text("host_name", "azurecr.io")
                    .with_text("thunk_symbol", "mirror");
            }));
            if let Some(_) = self.runtime_editor.runtime().schedule_with_engine::<Call, AppHost<Mirror>>(app_world, "test") {

            }
        }
    }
}

impl AsRef<Runtime> for Upstream {
    fn as_ref(&self) -> &Runtime {
        self.runtime_editor.runtime()
    }
}