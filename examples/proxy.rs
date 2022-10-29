use lifec::{
    prelude::{Editor, Host, Sequencer},
    project::{Project, RunmdFile, Workspace, Listener},
};
use lifec_registry::{RegistryProxy};

fn main() {
    tracing_subscriber::fmt::Subscriber::builder()
    .compact()
    .init();

    let mut workspace = Workspace::new("azurecr.io", None).tenant("obddemo2");
    workspace.set_root_runmd(
        r#"
    ```
    + .config start.mirror
    : skip_hosts_dir_check .true

    + .operation resolve.test
    : .install  access_token
    : .login    access_token
    : .authn    
    : .println Resolving {REGISTRY_HOST} {REGISTRY_TENANT} {REFERENCE} {REGISTRY_NAMESPACE} {WORK_DIR} {REGISTRY_REPO} {api} {Authorization}
    : .fmt REGISTRY_HOST, REGISTRY_TENANT, REFERENCE, REGISTRY_NAMESPACE, WORK_DIR, REGISTRY_REPO, api, Authorization
    : .chaos
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
        : .start    start
        : .start    recover
        : .loop
        ```

        ``` start
        + .runtime
        : .mirror    
        : .host         localhost:8578, resolve, pull
        
        + .proxy        localhost:8578
        : .manifests    
        : .get          resolve.test
        ```

        ``` recover
        + .runtime
        : .println Waiting fot 10 secs before repeating
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
    let world = RegistryProxy::compile_workspace(&workspace, files.iter(), None);

    let mut host = Host::from(world);
    host.link_sequences();
    host.enable_listener::<Test>();
    host.open_runtime_editor::<RegistryProxy>()
}

#[derive(Default)]
struct Test;

impl Listener for Test {
    fn create(_world: &specs::World) -> Self {
        Test {}
    }

    fn on_runmd(&mut self, _runmd: &RunmdFile) {
    }

    fn on_status_update(&mut self, _status_update: &lifec::prelude::StatusUpdate) {
    }

    fn on_operation(&mut self, _operation: lifec::prelude::Operation) {
    }

    fn on_error_context(&mut self, _error: &lifec::prelude::ErrorContext) {
    }

    fn on_completed_event(&mut self, _entity: &specs::Entity) {
    }

    fn on_start_command(&mut self, _start_command: &lifec::prelude::Start) {
    }
}