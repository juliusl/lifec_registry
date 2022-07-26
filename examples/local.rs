use std::env::args;

use lifec::{Runtime, editor::{RuntimeEditor, Call}, open, combine, combine_default, plugins::Project, App, System, start};
use lifec_registry::{Upstream, Mirror, create_runtime};
use tracing_subscriber::EnvFilter;

/// Example w/ local registry
fn main() {
    tracing_subscriber::fmt::Subscriber::builder()
        .with_env_filter(EnvFilter::from_default_env())
        .compact()
        .init();

    if let Some(project) = Project::runmd() {
        let runtime = RuntimeEditor::new(create_runtime(project));

        let args = args().collect::<Vec<_>>();

        if let Some(_) = args.iter().find(|a| a.starts_with("--mirror")) {
            start(Upstream::from(runtime), &[ "example" ]);
        } else {
            open(
                "example",
                Empty{},
                Upstream::from(runtime),
            )
        }
    }
}


struct Empty; 

impl App for Empty {
    fn name() -> &'static str {
        "empty"
    }

    fn enable_depth_stencil<'a>(&self) -> bool {
        true
    }

    fn edit_ui(&mut self, _ui: &imgui::Ui) {
    }

    fn display_ui(&self, _ui: &imgui::Ui) {
    }
}

impl<'a> System<'a> for Empty {
    type SystemData = ();

    fn run(&mut self, _: Self::SystemData) {
       
    }
}
