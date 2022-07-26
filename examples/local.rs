use std::env::args;

use lifec::{editor::{RuntimeEditor}, open, plugins::{Project, ThunkContext, Plugin}, App, System, start};
use lifec_registry::{Upstream, create_runtime, MirrorEvent};
use tracing_subscriber::EnvFilter;

/// Example w/ local registry
fn main() {
    tracing_subscriber::fmt::Subscriber::builder()
        .with_env_filter(EnvFilter::from_default_env())
        .compact()
        .init();

    if let Some(project) = Project::runmd() {
        let runtime = RuntimeEditor::new(create_runtime::<Empty>(project));

        let args = args().collect::<Vec<_>>();

        if let Some(_) = args.iter().find(|a| a.starts_with("--mirror")) {
            start(Upstream::<Empty>::from(runtime), &[ "example" ]);
        } else {
            open(
                "example",
                Empty{},
                Upstream::<Empty>::from(runtime),
            )
        }
    }
}

#[derive(Default)]
struct Empty; 

impl MirrorEvent for Empty {
    fn resolve_response(_tc: &lifec::plugins::ThunkContext) -> poem::Response {
        todo!()
    }

    fn resolve_error(_err: String, _tc: &lifec::plugins::ThunkContext) -> poem::Response {
        todo!()
    }
}

impl Plugin<ThunkContext> for Empty {
    fn symbol() -> &'static str {
        todo!()
    }

    fn call_with_context(_context: &mut ThunkContext) -> Option<lifec::plugins::AsyncContext> {
        todo!()
    }
}

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
