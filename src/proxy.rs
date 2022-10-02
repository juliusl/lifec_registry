use lifec::{
    default_parser, default_runtime, AttributeIndex, BlockIndex, CustomAttribute, Project,
    Runtime, SpecialAttribute, ThunkContext, Value, AttributeGraph, World, WorldExt,
};
use lifec_poem::WebApp;

mod methods;
use methods::Methods;

mod resources;
use resources::Resources;

use crate::{Authenticate, Discover, Login, Resolve};

/// Struct for creating a customizable registry proxy,
///
#[derive(Default)]
pub struct Proxy {
    context: ThunkContext,
}

impl Proxy {
    pub fn extract_routes(world: &World, index: &BlockIndex) {
        let graph = AttributeGraph::new(index.clone());

        if let Some(proxy_entity) = graph.find_int("proxy_entity") {
            let original = graph.entity_id();
            let proxy_entity = graph
                .scope(world.entities().entity(proxy_entity as u32))
                .expect("proxy entity should have been placed in the child properties");

            for route in proxy_entity.find_values("route") {
                match route {
                    Value::Int(id) if id as u32 != original => {
                        let graph = graph
                            .scope(world.entities().entity(id as u32))
                            .expect("should be a route");
                        for (name, value) in graph.values() {
                            eprintln!("{name}\n\t{:#?}", value);
                        }
                    }
                    _ => continue,
                }
            }

            let graph = graph.unscope();
            for (name, value) in graph.values() {
                eprintln!("{name}\n\t{:#?}", value);
            }
        }
    }
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
    fn interpret(world: &lifec::World, block: &lifec::Block) {
        for index in block.index().iter().filter(|b| b.root().name() == "proxy") {
            Proxy::extract_routes(world, index)
        }
    }

    fn parser() -> lifec::Parser {
        default_parser(Self::world()).with_special_attr::<Proxy>()
    }

    fn runtime() -> lifec::Runtime {
        let mut runtime = default_runtime();
        runtime.install_with_custom::<Login>("");
        runtime.install_with_custom::<Authenticate>("");
        runtime.install_with_custom::<Resolve>("");
        runtime.install_with_custom::<Discover>("");
        runtime
    }
}

impl WebApp for Proxy {
    fn create(context: &mut lifec::ThunkContext) -> Self {
        Self::from(context.clone())
    }

    fn routes(&mut self) -> poem::Route {
        todo!()
    }
}

impl From<ThunkContext> for Proxy {
    fn from(context: ThunkContext) -> Self {
        Self { context }
    }
}

mod tests {
    #[test]
    fn test_proxy_parsing() {
        use lifec::prelude::*;

        use crate::Proxy;
        let host = Host::load_content::<Proxy>(
            r#"
        # Example proxy definition
        ``` start proxy
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
        # :   .teleport               overlaybd, auto
        # :   .converter              convert overlaybd <name of the engine that can do the conversion>

        ## Validate signatures
        # :   .notary

        ## Download blobs
        : .blobs head, get
        : .login                  access_token
        : .authn                  oauth2
        : .println
        ```
        "#,
        );

        let block = Engine::find_block(host.world(), "start proxy").expect("block is created");

        let blocks = host.world().read_component::<Block>();
        if let Some(block) = blocks.get(block) {
            for index in block.index() {
                Proxy::extract_routes(host.world(), &index);
            }
        }
    }
}
