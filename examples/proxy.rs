use std::ops::Deref;

use lifec::{
    prelude::{Editor, Host, Sequencer, Appendix, WorkspaceEditor, Interpreter},
    project::{Listener, Project, RunmdFile, Workspace},
};
use lifec_registry::RegistryProxy;
use shinsu::{NodeExtension, SingleIO};
use specs::WorldExt;
use tracing_subscriber::EnvFilter;

fn main() {
    tracing_subscriber::fmt::Subscriber::builder()
        .with_env_filter(
            EnvFilter::builder()
                .from_env()
                .expect("should work"),
        )
        .compact()
        .init();

    let mut workspace = Workspace::new("azurecr.io", None).tenant("obddemo2");
    workspace.set_root_runmd(
        r#"
    
    # Implementation that will be executed when proxying the request
    ```
    + .config start.mirror
    : skip_hosts_dir_check .true

    + .operation resolve.test
    : .install      access_token
    : .login        access_token
    : .authn    
    : .request
    # : .resolve
    # : .discover     teleport.link.v1
    # : .teleport     overlaybd

    + .operation download.test
    : .install      access_token
    : .login        access_token
    : .authn    
    : .request

    # : .process sh test.sh
    # : .env REGISTRY_HOST
    # : .env REGISTRY_USER
    # : .env REGISTRY_TOKEN
    # : .env REGISTRY_TENANT
    # : .env REGISTRY_REPO
    # : .env REFERENCE
    # : .env WORK_DIR
    ```
    
    # Test operation to call the mirror
    ```
    + .operation test
    : .process curl
    : .arg localhost:8578/v2/redis/manifests/6.0.2?ns=obddemo2.azurecr.io
    : .arg -v
    : .flag -X GET
    : .flag -H Accept:application/vnd.docker.distribution.manifest.v2+json
    : .redirect output.resp
    ```

    # Test operation to call the mirror to download blob
    ```
    + .operation test-download
    : .process curl
    : .arg localhost:8578/v2/redis/blobs/sha256:afb6ec6fdc1c3ba04f7a56db32c5ff5ff38962dc4cd0ffdef5beaa0ce2eb77e2?ns=obddemo2.azurecr.io
    : .arg -v
    : .flag -H Accept:application/vnd.docker.image.rootfs.diff.tar.gzip
    : .flag -o layer.tar.gzip
    ```
    "#,
    );

    let root_runmd_path = workspace.work_dir().join(".runmd");
    println!("{:?}", root_runmd_path);
    std::fs::create_dir_all(workspace.work_dir()).ok();
    std::fs::write(
        root_runmd_path,
        workspace.root_runmd().expect("should have a value"),
    )
    .ok();

    let mirror = RunmdFile::new_src(
        "mirror",
        r#"
        ```
        + .engine
        : .start    setup
        : .start    start
        : .start    recover
        : .loop
        ```

        ``` setup
        + .runtime
        : .login_acr
        ```

        ``` start

        + .runtime
        : .mirror    
        : .host         localhost:8578, resolve, pull
        
        + .proxy        localhost:8578
        : .manifests    
        : .get          resolve.test
        : .blobs
        : .get          download.test
        ```

        ``` recover
        + .runtime
        : .println Waiting for 10 secs before repeating
        : .timer 10 s
        ```
        "#,
    );

    let mirror_runmd_path = workspace.work_dir().join("mirror.runmd");
    println!("{:?}", mirror_runmd_path);
    std::fs::create_dir_all(workspace.work_dir()).ok();
    std::fs::write(
        mirror_runmd_path,
        mirror.source.clone().expect("should have a value"),
    )
    .ok();

    let files = vec![mirror];

    // Manually compile workspace since we don't need settings from the CLI --
    let mut world = RegistryProxy::compile_workspace(&workspace, files.iter(), None);

    let node_editor = NodeExtension::new(SingleIO::default());
    node_editor.initialize(&mut world);

    let mut host = Host::from(world);
    host.link_sequences();
    host.enable_listener::<Test>();
    host.build_appendix();
    let appendix = host.world().read_resource::<Appendix>().deref().clone();
    let workspace_editor = WorkspaceEditor::from(appendix);
    host.open::<RegistryProxy, _>((workspace_editor, node_editor));
}

#[derive(Default)]
struct Test;

impl Listener for Test {
    fn create(_world: &specs::World) -> Self {
        Test {}
    }

    fn on_runmd(&mut self, _runmd: &RunmdFile) {}

    fn on_status_update(&mut self, _status_update: &lifec::prelude::StatusUpdate) {}

    fn on_operation(&mut self, _operation: lifec::prelude::Operation) {}

    fn on_error_context(&mut self, _error: &lifec::prelude::ErrorContext) {}

    fn on_completed_event(&mut self, _entity: &specs::Entity) {}

    fn on_start_command(&mut self, _start_command: &lifec::prelude::Start) {}
}
