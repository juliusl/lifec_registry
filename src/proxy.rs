use lifec::{
    AttributeIndex, CustomAttribute, Host, Project, Runtime, SpecialAttribute, ThunkContext, Value,
};
use lifec_poem::WebApp;
use poem::Route;

mod methods;
use methods::Methods;

mod resources;
use resources::Resources;

/// Struct for creating a customizable registry proxy,
///
#[derive(Default)]
pub struct Proxy {
    context: ThunkContext,
}

impl SpecialAttribute for Proxy {
    fn ident() -> &'static str {
        "proxy"
    }

    /// This alias is so that `.proxy` stable attributes are not interpreted
    /// by the normal `.engine` interpreter. However, we still want access to the world's runtime
    /// on `parse()`
    ///
    fn parse(parser: &mut lifec::AttributeParser, content: impl AsRef<str>) {
        parser.define("app_host", Value::Symbol(content.as_ref().to_string()));

        Runtime::parse(parser, &content);
        parser.add_custom(CustomAttribute::new_with("manifests", |p, c| {
            Methods::parse_methods(Resources::Manifests)(p, c);
        }));
        parser.add_custom(CustomAttribute::new_with("blobs", |p, c| {
            Methods::parse_methods(Resources::Blobs)(p, c);
        }));
        parser.add_custom(CustomAttribute::new_with("tags", |p, c| {
            Methods::parse_methods(Resources::Tags)(p, c);
        }));
    }
}

impl Project for Proxy {
    fn configure_engine(_: &mut lifec::Engine) {
        //
    }

    fn interpret(_: &lifec::World, block: &lifec::Block) {
        for index in block.index().iter().filter(|b| b.root().name() == "proxy") {
            // for route in index
            //     .properties()
            //     .property("route")
            //     .and_then(|p| p.symbol_vec())
            //     .expect("should be a symbol vector")
            // {
            // }
        }
    }

    fn configure_dispatcher(
        dispatcher_builder: &mut lifec::DispatcherBuilder,
        context: Option<ThunkContext>,
    ) {
        if let Some(tc) = context {
            Host::add_error_context_listener::<Proxy>(tc, dispatcher_builder);
        }
    }

    fn on_error_context(&mut self, _error: lifec::plugins::ErrorContext) {}
}

impl WebApp for Proxy {
    fn create(context: &mut lifec::ThunkContext) -> Self {
        Self::from(context.clone())
    }

    fn routes(&mut self) -> poem::Route {
        self.context.state().find_bool("todo");
        /*
        Sketching up initial design
        ``` start proxy
        + .runtime
        : .login-acr              access_token
        : .println                Starting proxy
        : .mirror                 {registry_name}.{registry_host}
        : .host                   localhost:8567, resolve, pull

        # Proxy setup
        + .proxy                  localhost:8567

        ## Resolve manifests and artifacts
        : .manifests head, get
        :   .login                  access_token
        :   .authn                  oauth2
        :   .resolve                application/vnd.oci.image.manifest.v1+json <if accept is * or matches>
        :   .discover               dadi.image.v1
        :   .discover               sbom.json

        ## Teleport and dispatch a convert operation if teleport isn't available
        :   .teleport               overlaybd, auto
        :   .converter              convert overlaybd <name of the engine that can do the conversion>

        ## Validate signatures
        :   .notary

        ## Download blobs
        : .blobs head, get
        :   .login                  access_token
        :   .authn                  oauth2
        :   .pull
        ```
        */

        Route::new()
    }
}

impl From<ThunkContext> for Proxy {
    fn from(context: ThunkContext) -> Self {
        Self { context }
    }
}
