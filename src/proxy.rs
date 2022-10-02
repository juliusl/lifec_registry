use lifec::{
    AttributeIndex, CustomAttribute, Host, Project, Runtime, SpecialAttribute, ThunkContext,
};
use lifec_poem::WebApp;
use poem::Route;

mod methods;
use methods::Methods;

/// Struct for creating a customizable registry proxy,
///
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
        Runtime::parse(parser, content);
        parser.add_custom(CustomAttribute::new_with("manifests", |p, c| {
            Methods::parse_methods("manifests")(p, c)
        }));
        parser.add_custom(CustomAttribute::new_with("blobs", |p, c| {
            Methods::parse_methods("blobs")(p, c)
        }));
        parser.add_custom(CustomAttribute::new_with("tags", |p, c| {
            Methods::parse_methods("tags")(p, c)
        }));
    }
}

impl Project for Proxy {
    fn configure_engine(_: &mut lifec::Engine) {
        //
    }

    fn interpret(_: &lifec::World, _: &lifec::Block) {
        //
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
        Self {
            context: context.clone(),
        }
    }

    fn routes(&mut self) -> poem::Route {
        self.context.state().find_bool("todo");
        /*
        Sketching up initial design
        ``` start proxy
        + .runtime
        : .acr-login              access_token
        : <username> .acr-login   access_token
        : .println                Starting proxy
        : .mirror                 <>
        : .host                   localhost:8567

        # Proxy setup
        + .proxy                  localhost:8567

        ## Resolve manifests and artifacts
        : .manifests head, get
        :   .login                  access_token
        :   .authn                  oauth2
        :   .resolve                application/vnd.oci.image.manifest.v1+json <if accept is * or matches>
        :   .artifact               dadi.image.v1

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
