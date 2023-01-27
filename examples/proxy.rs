use lifec::{
    prelude::{Editor, Host, Sequencer},
    project::{Project, RunmdFile, Workspace},
};
use lifec_registry::RegistryProxy;
use tracing_subscriber::EnvFilter;

fn main() {
    std::env::set_var("RUST_LOG", "reality=debug,reality_azure=debug,lifec_registry=debug,lifec=trace");

    tracing_subscriber::fmt::Subscriber::builder()
        .with_env_filter(
            EnvFilter::builder()
            .from_env()
            .expect("should work"),
        )
        .compact()
        .init();

    std::env::set_var("ACCOUNT_NAME", "obddemospace");

    let mut workspace = Workspace::new("azurecr.io", None).tenant("obddemo2");
    workspace.set_root_runmd(
        r#"
    # Implementation that will be executed when proxying the request
    ```
    + .config start.mirror
    : skip_hosts_dir_check .true
    : enable_guest_agent   .true

    + .operation resolve.test
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

    + .operation resolve.test2
    : .login_acr
    : .install      access_token
    : .login        access_token
    : .authn        https://obddemo2.azurecr.io/v2/d/redis/manifests/6.0.2
    : .method       GET
    : .request      
    : .accept       application/vnd.docker.distribution.manifest.v2+json
    : .store

    + .operation open_remote_registry
    : .remote_registry obddemospace
    : .remote_guest

    + .operation setup_remote_registry
    : .remote_registry obddemospace
    : .process sh setup-guest-storage.sh
    ```
    
    # Test println
    ```
    + .operation test-println
    : .println Hello World
    ```

    # Test operation to call the mirror
    ```
    + .operation test
    : .process curl
    : .arg localhost:8578/v2/d/redis/manifests/6.0.2?ns=obddemo2.azurecr.io
    : .arg -silent
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
    let world = RegistryProxy::compile_workspace(&workspace, files.iter(), None);

    let mut host = Host::from(world);
    host.link_sequences();
    host.open_runtime_editor::<RegistryProxy>(true)
}
