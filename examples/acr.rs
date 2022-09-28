// use std::env::args;
// use lifec::{plugins::{ThunkContext, Plugin}, App, System, start};
// use lifec_registry::{Upstream, create_runtime, MirrorEvent};
// use poem::{Response, http::StatusCode};
// use tracing_subscriber::EnvFilter;

// /// Example w/ local registry
// fn main() {
//     tracing_subscriber::fmt::Subscriber::builder()
//         .with_env_filter(EnvFilter::from_default_env())
//         .compact()
//         .init();

//     if let Some(project) = Project::load_file("examples/acr/.runmd") {
//         let runtime = RuntimeEditor::new(create_runtime::<ACR>(project));

//         let args = args().collect::<Vec<_>>();

//         if let Some(_) = args.iter().find(|a| a.starts_with("--mirror")) {
//             start(Upstream::<ACR>::from(runtime), &[ "example" ]);
//         } else {
//             open(
//                 "example",
//                 ACR{},
//                 Upstream::<ACR>::from(runtime),
//             )
//         }
//     }
// }

// #[derive(Default)]
// struct ACR; 

// impl MirrorEvent for ACR {
//     fn resolve_response(tc: &lifec::plugins::ThunkContext) -> poem::Response {
//         if let Some(thunk_symbol) = tc.as_ref().find_text("thunk_symbol") {
//             match thunk_symbol.as_str() {
//                 "resolve" => {
                    
//                 }
//                 "download_blob" => {

//                 }
//                 "list_tags" => {
                    
//                 }
//                 "blob_import" => {

//                 }
//                 "blob_upload_chunks" => {

//                 }
//                 "blob_upload_session_id" => {

//                 }
//                 _ =>  {
//                 }
//             }
//         }

//         Response::builder()
//             .finish()
//     }

//     fn resolve_error(_err: String, _tc: &lifec::plugins::ThunkContext) -> poem::Response {
//         Response::builder()
//             .status(StatusCode::SERVICE_UNAVAILABLE)
//             .finish()
//     }
// }

// impl Plugin for ACR {
//     fn symbol() -> &'static str {
//         "example_acr"
//     }

//     fn description() -> &'static str {
//         "This will login to acr and make the authenticated request"
//     }

//     fn call(context: &ThunkContext) -> Option<lifec::plugins::AsyncContext> {
//         context.clone().task(|_| {
//             let tc = context.clone();
//             async move {
//                 if let Some(client) = tc.client() {
//                 }

//                 None 
//             }
//         })
//     }
// }

// impl App for ACR {
//     fn name() -> &'static str {
//         "empty"
//     }

//     fn enable_depth_stencil<'a>(&self) -> bool {
//         true
//     }

//     fn edit_ui(&mut self, _ui: &imgui::Ui) {
//     }

//     fn display_ui(&self, _ui: &imgui::Ui) {
//     }
// }

// impl<'a> System<'a> for ACR {
//     type SystemData = ();

//     fn run(&mut self, _: Self::SystemData) {
       
//     }
// }

fn main() {
    
}