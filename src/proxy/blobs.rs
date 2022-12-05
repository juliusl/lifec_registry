use std::sync::Arc;

use hyper::Method;
use lifec::prelude::{AttributeParser, Host, SpecialAttribute, Value, ThunkContext};
use lifec_poem::RoutePlugin;
use poem::{
    delete, get, handler, head,
    web::{Data, Path, Query},
    EndpointExt, Response, RouteMethod,
};
use serde::{Deserialize, Serialize};
use specs::{Component, VecStorage, WorldExt};
use tracing::{event, Level};

use crate::Registry;

/// Route plugin to handle registry resolve requests,
///
/// Example:
/// : .mirror     <azurecr.io>
/// : .host       <address> resolve, push
///
/// + .proxy      <address>
/// : .blobs  
/// : .get        <operation-name>
/// : .head       <operation-name>
///
#[derive(Component, Default, Clone, Serialize, Deserialize)]
#[storage(VecStorage)]
pub struct Blobs {
    /// Upstream namespace,
    ns: String,
    /// Method to proxy,
    #[serde(skip)]
    method: Option<Method>,
    /// Workspace operation to execute before completing the response,
    #[serde(skip)]
    operation: Option<String>,
    /// Command dispatcher,
    #[serde(skip)]
    context: ThunkContext,
    /// Host,
    #[serde(skip)]
    host: Arc<Host>
}

impl Blobs {
    /// Returns true if this component can be routed,
    /// 
    pub fn can_route(&self) -> bool {
        self.method.is_some() && self.operation.is_some()
    }

    /// Sets the host,
    /// 
    pub fn set_host(&mut self, host: Arc<Host>) {
        self.host = host;
    }

    /// Sets the context,
    /// 
    pub fn set_context(&mut self, context: ThunkContext) {
        self.context = context;
    }
}

impl SpecialAttribute for Blobs {
    fn ident() -> &'static str {
        "blobs"
    }

    fn parse(parser: &mut AttributeParser, content: impl AsRef<str>) {
        let world = parser.world().expect("should have a world");

        let blobs = Blobs::default();

        let proxy_entity = world.entities().create();
        world
            .write_component()
            .insert(proxy_entity, blobs)
            .expect("should be able to insert component");

        parser.define_child(
            proxy_entity,
            "proxy",
            Value::Symbol(content.as_ref().to_string()),
        );

        parser.add_custom_with("get", |p, c| {
            let last_entity = p.last_child_entity().expect("should have an entity");
            let world = p.world().expect("should have a world");

            let blobs = {
                let blobs = world.read_component::<Blobs>();
                let blobs = blobs.get(last_entity).expect("should have a blob");
                blobs.clone()
            };

            let mut route = blobs.clone();
            route.method = Some(Method::GET);
            route.operation = Some(c);
            let route_entity = world.entities().create();
            world
                .write_component()
                .insert(route_entity, route)
                .expect("should be able to insert component");
        });

        parser.add_custom_with("head", |p, c| {
            let last_entity = p.last_child_entity().expect("should have an entity");
            let world = p.world().expect("should have a world");

            if let Some(blobs) = world.read_component::<Blobs>().get(last_entity) {
                let mut route = blobs.clone();
                route.method = Some(Method::HEAD);
                route.operation = Some(c);
                let route_entity = world.entities().create();
                world
                    .write_component()
                    .insert(route_entity, route)
                    .expect("should be able to insert component");
            }
        });

        parser.add_custom_with("delete", |p, c| {
            let last_entity = p.last_child_entity().expect("should have an entity");
            let world = p.world().expect("should have a world");

            if let Some(blobs) = world.read_component::<Blobs>().get(last_entity) {
                let mut route = blobs.clone();
                route.method = Some(Method::DELETE);
                route.operation = Some(c);
                let route_entity = world.entities().create();
                world
                    .write_component()
                    .insert(route_entity, route)
                    .expect("should be able to insert component");
            }
        });
    }
}

impl RoutePlugin for Blobs {
    fn route(&self, mut route: Option<RouteMethod>) -> RouteMethod {
        let path = "/:repo<[a-zA-Z0-9/_-]+(?:blobs)>/:reference";

        if let Some(route) = route.take() {
            match self.method {
                Some(Method::GET) => {
                    event!(Level::DEBUG, "adding path GET {path}");
                    route.get(blobs_api.data(self.clone()).data(self.host.clone()).data(self.context.clone()))
                }
                Some(Method::HEAD) => {
                    event!(Level::DEBUG, "adding path HEAD {path}");
                    route.head(blobs_api.data(self.clone()).data(self.host.clone()).data(self.context.clone()))
                }
                Some(Method::DELETE) => {
                    event!(Level::DEBUG, "adding path DELETE {path}");
                    route.delete(blobs_api.data(self.clone()).data(self.host.clone()).data(self.context.clone()))
                }
                _ => {
                    panic!("Unsupported method, {:?}", self.method)
                }
            }
        } else {
            match self.method {
                Some(Method::GET) => {
                    event!(Level::DEBUG, "adding path GET {path}");
                    get(blobs_api.data(self.clone()).data(self.host.clone()).data(self.context.clone()))
                }
                Some(Method::HEAD) => {
                    event!(Level::DEBUG, "adding path HEAD {path}");
                    head(blobs_api.data(self.clone()).data(self.host.clone()).data(self.context.clone()))
                }
                Some(Method::DELETE) => {
                    event!(Level::DEBUG, "adding path DELETE {path}");
                    delete(blobs_api.data(self.clone()).data(self.host.clone()).data(self.context.clone()))
                }
                _ => {
                    panic!("Unsupported method, {:?}", self.method)
                }
            }
        }
    }

    fn response(context: &mut lifec::prelude::ThunkContext) -> Response {
        if let Some(response) = context.take_response() {
            response.into()
        } else {
            Registry::soft_fail()
        }
    }
}

#[handler]
async fn blobs_api(
    request: &poem::Request,
    Path((repo, reference)): Path<(String, String)>,
    Query(Blobs { ns, .. }): Query<Blobs>,
    blobs: Data<&Blobs>,
    host: Data<&Arc<Host>>,
    context: Data<&ThunkContext>,
) -> Response {
    let mut registry = host.world().system_data::<Registry>();

    registry
        .proxy_request::<Blobs>(
            &context,
            blobs.operation
                .clone()
                .expect("should have an operation name"),
            request,
            None,
            ns,
            repo.trim_end_matches("/blobs"),
            reference,
        ).await
}
