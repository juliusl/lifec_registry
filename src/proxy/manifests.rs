use std::sync::Arc;

use hyper::Method;
use lifec::prelude::{AttributeParser, Host, SpecialAttribute, Value, ThunkContext};
use lifec_poem::RoutePlugin;
use poem::{
    delete, get, handler, head,
    web::{Data, Path, Query},
    EndpointExt, Response, RouteMethod, post, Body,
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
/// : .manifests  
/// : .get        <operation-name>
/// : .head       <operation-name>
///
#[derive(Component, Default, Clone, Serialize, Deserialize)]
#[storage(VecStorage)]
pub struct Manifests {
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

impl Manifests {
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

impl SpecialAttribute for Manifests {
    fn ident() -> &'static str {
        "manifests"
    }

    fn parse(parser: &mut AttributeParser, content: impl AsRef<str>) {
        let world = parser.world().expect("should have a world");

        let manifests = Manifests::default();

        let proxy_entity = world.entities().create();
        world
            .write_component()
            .insert(proxy_entity, manifests)
            .expect("should be able to insert component");

        parser.define_child(
            proxy_entity,
            "proxy",
            Value::Symbol(content.as_ref().to_string()),
        );

        parser.add_custom_with("get", |p, c| {
            let last_entity = p.last_child_entity().expect("should have an entity");
            let world = p.world().expect("should have a world");

            let manifests = {
                let manifests = world.read_component::<Manifests>();
                let manifests = manifests.get(last_entity).expect("should have a manifest");
                manifests.clone()
            };

            let mut route = manifests.clone();
            route.method = Some(Method::GET);
            route.operation = Some(c);
            let route_entity = world.entities().create();
            world
                .write_component()
                .insert(route_entity, route)
                .expect("should be able to insert component");
        });

        parser.add_custom_with("post", |p, c| {
            let last_entity = p.last_child_entity().expect("should have an entity");
            let world = p.world().expect("should have a world");

            let manifests = {
                let manifests = world.read_component::<Manifests>();
                let manifests = manifests.get(last_entity).expect("should have a manifest");
                manifests.clone()
            };

            let mut route = manifests.clone();
            route.method = Some(Method::POST);
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

            if let Some(manifests) = world.read_component::<Manifests>().get(last_entity) {
                let mut route = manifests.clone();
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

            if let Some(manifests) = world.read_component::<Manifests>().get(last_entity) {
                let mut route = manifests.clone();
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

impl RoutePlugin for Manifests {
    fn route(&self, mut route: Option<RouteMethod>) -> RouteMethod {
        let path = "/:repo<[a-zA-Z0-9/_-]+(?:manifests)>/:reference";

        if let Some(route) = route.take() {
            match self.method {
                Some(Method::GET) => {
                    event!(Level::DEBUG, "adding path GET {path}");
                    route.get(manifest_api.data(self.clone()).data(self.host.clone()).data(self.context.clone()))
                }
                Some(Method::POST) => {
                    event!(Level::DEBUG, "adding path POST {path}");
                    route.post(manifest_api.data(self.clone()).data(self.host.clone()).data(self.context.clone()))
                }
                Some(Method::HEAD) => {
                    event!(Level::DEBUG, "adding path HEAD {path}");
                    route.head(manifest_api.data(self.clone()).data(self.host.clone()).data(self.context.clone()))
                }
                Some(Method::DELETE) => {
                    event!(Level::DEBUG, "adding path DELETE {path}");
                    route.delete(manifest_api.data(self.clone()).data(self.host.clone()).data(self.context.clone()))
                }
                _ => {
                    panic!("Unsupported method, {:?}", self.method)
                }
            }
        } else {
            match self.method {
                Some(Method::GET) => {
                    event!(Level::DEBUG, "adding path GET {path}");
                    get(manifest_api.data(self.clone()).data(self.host.clone()).data(self.context.clone()))
                }
                Some(Method::POST) => {
                    event!(Level::DEBUG, "adding path POST {path}");
                    post(manifest_api.data(self.clone()).data(self.host.clone()).data(self.context.clone()))
                }
                Some(Method::HEAD) => {
                    event!(Level::DEBUG, "adding path HEAD {path}");
                    head(manifest_api.data(self.clone()).data(self.host.clone()).data(self.context.clone()))
                }
                Some(Method::DELETE) => {
                    event!(Level::DEBUG, "adding path DELETE {path}");
                    delete(manifest_api.data(self.clone()).data(self.host.clone()).data(self.context.clone()))
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
async fn manifest_api(
    request: &poem::Request,
    body: Body,
    Path((repo, reference)): Path<(String, String)>,
    Query(Manifests { ns, .. }): Query<Manifests>,
    resolve: Data<&Manifests>,
    host: Data<&Arc<Host>>,
    context: Data<&ThunkContext>,
) -> Response {
    let mut registry = host.world().system_data::<Registry>();

    registry
        .proxy_request::<Manifests>(
            &context,
            resolve
                .operation
                .clone()
                .expect("should have an operation name"),
            request,
            Some(body.into()),
            ns,
            repo.trim_end_matches("/manifests"),
            reference,
        ).await
}
