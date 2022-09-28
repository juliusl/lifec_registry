use hyper::StatusCode;
use lifec::{Project, Interpreter, Host, default_runtime, WorldExt, Event, ThunkContext};
use lifec_registry::{Mirror, MirrorProxy};
use poem::Response;
use tracing_subscriber::EnvFilter;

/// Small example tool to convert .runmd to hosts.toml
/// 
fn main() {
    tracing_subscriber::fmt::Subscriber::builder()
        .with_env_filter(EnvFilter::from_default_env())
        .compact()
        .init();

    let mut host = Host::load_content::<Parse>(r#"
    ``` containerd
    + .engine
    : .event test
    : .exit
    ```
    
    ``` test containerd

    + .runtime
    :  src_dir  .symbol .
    :  work_dir .symbol .work/acr

    : .process  sh lib/login-acr.sh
    :  REGISTRY_NAME .env obddemo

    : .install  access_token
    
    : .mirror   azurecr.io
    : .server   https://test.azurecr.io
    : .host     localhost:5049, resolve
    : .host     localhost:3033, pull, push
    ```
    "#);

    let mut dispatcher = {
        let dispatcher = Host::dispatcher_builder();
        dispatcher.build()
    };
    dispatcher.setup(host.world_mut());
    
    // TODO - Turn this into an api
    let event = host.world().entities().entity(3);
    if let Some(event) = host.world().write_component::<Event>().get_mut(event) {
        event.fire(ThunkContext::default());
    }
    host.world_mut().maintain();

    while !host.should_exit() {
        dispatcher.dispatch(host.world());
    }
}

#[derive(Debug, Default)]
struct Parse;

impl Project for Parse {
    fn configure_engine(_: &mut lifec::Engine) {
        // No-op
    }

    fn interpret(world: &lifec::World, block: &lifec::Block) {
        Mirror::<Self>::default().interpret(world, block)
    }

    fn runtime() -> lifec::Runtime {
        let mut runtime = default_runtime(); 
        runtime.install_with_custom::<Mirror<Self>>("");
        runtime
    }
}

/// Unused
/// 
impl MirrorProxy for Parse {
    fn resolve_response(_: &lifec::ThunkContext) -> poem::Response {
        Response::builder().status(StatusCode::OK).finish()
    }

    fn resolve_error(_: String, _: &lifec::ThunkContext) -> poem::Response {
        Response::builder().status(StatusCode::OK).finish()
    }
}