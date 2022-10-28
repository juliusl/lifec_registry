use lifec::{
    prelude::{Editor, Host, Sequencer},
    project::{Project, RunmdFile, Workspace},
};
use lifec_registry::RegistryProxy;

fn main() {
    let mut workspace = Workspace::new("azurecr.io", None).tenant("obddemo2");
    workspace.set_root_runmd(
        r#"
    ```
    + .operation resolve.test
    : .println Resolving
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
        ```

        ``` start
        + .runtime
        : .mirror    
        : .host         localhost:8578, resolve, pull
        
        + .proxy        localhost:8578
        : .manifests    
        : .get          resolve.test
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
    host.open_runtime_editor::<RegistryProxy>()
}
