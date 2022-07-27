use lifec::{Runtime, editor::{Call, RuntimeEditor, Fix}, Extension, plugins::{Project, Expect, Missing, Config}};

mod mirror;
pub use mirror::Mirror;
pub use mirror::MirrorEvent;
pub use mirror::MirrorHost;
pub use mirror::MirrorAction;

mod authenticate;
pub use authenticate::Authenticate;

mod login;
pub use login::Login;

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

/// Returns a runtime w/ plugins installed for use w/ lifec_registry
pub fn create_runtime<Event>(project: Project) -> Runtime 
where
    Event: MirrorEvent + Default + Send + Sync + 'static
{
    let mut runtime = Runtime::new(project);
    runtime.install::<Call, BlobImport>();
    runtime.install::<Call, BlobUploadChunks>();
    runtime.install::<Call, BlobUploadMonolith>();
    runtime.install::<Call, BlobUploadSessionId>();
    runtime.install::<Call, DownloadBlob>();
    runtime.install::<Call, ListTags>();
    runtime.install::<Call, Resolve>();
    runtime.install::<Call, Expect>();
    runtime.install::<Call, MirrorHost::<Event>>();
    runtime.install::<Fix, Missing>();
    runtime
}

/// Represents the upstream registry that is being extended
#[derive(Default)]
pub struct Upstream<Event> 
where
    Event: MirrorEvent + Default + Send + Sync + 'static
{
    runtime_editor: RuntimeEditor,
    host_name: String,
    event: Event,
}

impl<Event> From<RuntimeEditor> for Upstream<Event>
where
    Event: MirrorEvent + Default + Send + Sync + 'static
{
    fn from(runtime_editor: RuntimeEditor) -> Self {
        Self { runtime_editor, ..Default::default() }
    }
}

impl<Event> Extension for Upstream<Event>
where
    Event: MirrorEvent + Default + Send + Sync + 'static
{
    fn configure_app_world(world: &mut lifec::World) {
        RuntimeEditor::configure_app_world(world);
    }

    fn configure_app_systems(dispatcher: &mut lifec::DispatcherBuilder) {
        RuntimeEditor::configure_app_systems(dispatcher);
    }

    fn on_ui(&'_ mut self, app_world: &lifec::World, ui: &'_ imgui::Ui<'_>) {
        self.runtime_editor.on_ui(app_world, ui);

        if ui.button("test") {
            self.runtime_editor.runtime_mut().add_config(Config("test", |a| {
                a.block.block_name = a.label("test").as_ref().to_string();

                a.as_mut()
                    .with_text("project_src", "examples/.runmd")
                    .with_text("address", "localhost:5000")
                    .with_text("host_name", "azurecr.io")
                    .with_text("thunk_symbol", "mirror");
            }));
            if let Some(_) = self.runtime_editor.runtime()
                .schedule_with_engine::<Call, MirrorHost<Event>>(app_world, "test") {

            }
        }
    }

    fn on_window_event(&'_ mut self, app_world: &lifec::World, event: &'_ lifec::editor::WindowEvent<'_>) {
        self.runtime_editor.on_window_event(app_world, event)
    }

    fn on_run(&'_ mut self, app_world: &lifec::World) {
        self.runtime_editor.on_run(app_world);
    }
}

impl<Event> AsRef<Runtime> for Upstream<Event>
where
    Event: MirrorEvent + Default + Send + Sync + 'static
{
    fn as_ref(&self) -> &Runtime {
        self.runtime_editor.runtime()
    }
}