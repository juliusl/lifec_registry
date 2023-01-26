use std::{sync::Arc, marker::PhantomData};

use hyper::Method;
use lifec::prelude::{AttributeParser, Host, SpecialAttribute, Value, ThunkContext};
use lifec_poem::RoutePlugin;
use poem::{
    delete, get, handler, head, put, post,
    web::{Data, Path, Query},
    EndpointExt, Response, RouteMethod, Body, 
};
use serde::{Deserialize, Serialize};
use specs::{Component, VecStorage, WorldExt, Join};
use tracing::{event, Level, debug};

use crate::Registry;

/// Trait to include a specific route to the proxy,
/// 
pub trait RouteParameters: Default + Clone + Send + Sync + 'static {
    /// Returns a path,
    /// 
    fn path() -> &'static str;

    /// Returns the resource ident for this route,
    /// 
    fn ident() -> &'static str;
}

/// Trait for a fn that adds a new proxy route to an app,
/// 
pub trait AddRoute {
    /// Adds a proxy route to an app,
    /// 
    fn add_route<R>(self, host: &Arc<Host>, context: &ThunkContext) -> Self
    where 
        R: RouteParameters;
}

impl AddRoute for poem::Route {
    fn add_route<R>(mut self, host: &Arc<Host>, context: &ThunkContext) -> Self 
    where 
        R: RouteParameters
    {
        let mut proxy_route = None::<RouteMethod>;
        for r in host.world().read_component::<ProxyRoute<R>>().join() {
            if r.can_route() {
                let mut r = r.clone();
                r.set_context(context.clone());

                if let Some(m) = proxy_route.take() {
                    proxy_route = Some(r.route(Some(m)));
                } else {
                    proxy_route = Some(r.route(None));
                }
            }
        }
        let path = R::path();
        if let Some(proxy_route) = proxy_route.take() {
            self = self.at(path, proxy_route);
        }

        self
    }
}

/// Route plugin to handle registry resolve requests,
///
/// Example:
/// : .mirror     
/// : .host       <address> resolve, push
///
/// + .proxy      <address>
/// : .manifests  
/// : .get        <operation-name>
/// : .head       <operation-name>
/// : .blobs
/// : .get        <operation-name>
///
#[derive(Component, Default, Clone, Serialize, Deserialize)]
#[storage(VecStorage)]
pub struct ProxyRoute<R: RouteParameters> {
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
    #[serde(skip)]
    _r: PhantomData<R>,
}

impl<R: RouteParameters> ProxyRoute<R> {
    /// Returns true if this component can be routed,
    /// 
    fn can_route(&self) -> bool {
        self.method.is_some() && self.operation.is_some()
    }

    /// Sets the context,
    /// 
    fn set_context(&mut self, context: ThunkContext) {
        self.context = context;
    }
}

impl<R: RouteParameters> SpecialAttribute for ProxyRoute<R> {
    fn ident() -> &'static str {
        R::ident()
    }

    fn parse(parser: &mut AttributeParser, content: impl AsRef<str>) {
        let world = parser.world().expect("should have a world");

        let manifests = ProxyRoute::<R>::default();

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

        add::<R, GETKEY>(parser);
        add::<R, POSTKEY>(parser);
        add::<R, PUTKEY>(parser);
        add::<R, HEADKEY>(parser);
        add::<R, DELETEKEY>(parser);
    }
}

/// This is to keep the parse function DRY, otherwise a parse method would need to be written per method type
const CONNECTKEY: usize = 0;
const DELETEKEY: usize = 1;
const GETKEY: usize = 2;
const HEADKEY: usize = 3;
const OPTIONSKEY: usize = 4;
const PATCHKEY: usize = 5;
const POSTKEY: usize = 6;
const PUTKEY: usize = 7;
const TRACEKEY: usize = 8;

fn add<R: RouteParameters, const METHODKEY: usize>(parser: &mut AttributeParser) {
    parser.add_custom_with(ident::<METHODKEY>(), parse::<R, METHODKEY>);
}

fn ident<const METHODKEY: usize>() -> &'static str {
    match METHODKEY {
        CONNECTKEY => "connect",
        DELETEKEY => "delete",
        GETKEY => "get",
        HEADKEY => "head",
        OPTIONSKEY => "options",
        PATCHKEY => "patch",
        POSTKEY => "post",
        PUTKEY => "put",
        TRACEKEY => "trace",
        _ => unimplemented!()
    }
}

fn parse<R: RouteParameters, const METHODKEY: usize>(p: &mut AttributeParser, c: String) {
        let last_entity = p.last_child_entity().expect("should have an entity");
        let world = p.world().expect("should have a world");

        let route = {
            let route = world.read_component::<ProxyRoute<R>>();
            let route = route.get(last_entity).expect("should have a route component");
            route.clone()
        };

        let mut route = route.clone();
        route.method = match METHODKEY {
            CONNECTKEY => Some(Method::CONNECT),
            DELETEKEY => Some(Method::DELETE),
            GETKEY => Some(Method::GET),
            HEADKEY => Some(Method::HEAD),
            OPTIONSKEY => Some(Method::OPTIONS),
            PATCHKEY => Some(Method::PATCH),
            POSTKEY => Some(Method::POST),
            PUTKEY => Some(Method::PUT),
            TRACEKEY => Some(Method::TRACE),
            _ => None
        };
        route.operation = Some(c);
        let route_entity = world.entities().create();
        world
            .write_component()
            .insert(route_entity, route)
            .expect("should be able to insert component");
}

impl<R: RouteParameters> RoutePlugin for ProxyRoute<R> {
    fn route(&self, mut route: Option<RouteMethod>) -> RouteMethod {
        let path = R::path();
        let api = proxy_api::<R>::default()
            .data(self.clone())
            .data(Registry::default())
            .data(self.context.clone());

        if let Some(route) = route.take() {
            match self.method {
                Some(Method::GET) => {
                    event!(Level::DEBUG, "adding path GET {path}");
                    route.get(api)
                }
                Some(Method::POST) => {
                    event!(Level::DEBUG, "adding path POST {path}");
                    route.post(api)
                }
                Some(Method::PUT) => {
                    event!(Level::DEBUG, "adding path PUT {path}");
                    route.put(api)
                }
                Some(Method::HEAD) => {
                    event!(Level::DEBUG, "adding path HEAD {path}");
                    route.head(api)
                }
                Some(Method::DELETE) => {
                    event!(Level::DEBUG, "adding path DELETE {path}");
                    route.delete(api)
                }
                _ => {
                    unimplemented!("Unsupported method, {:?}", self.method)
                }
            }
        } else {
            match self.method {
                Some(Method::GET) => {
                    event!(Level::DEBUG, "adding path GET {path}");
                    get(api)
                }
                Some(Method::POST) => {
                    event!(Level::DEBUG, "adding path POST {path}");
                    post(api)
                }
                Some(Method::PUT) => {
                    event!(Level::DEBUG, "adding path PUT {path}");
                    put(api)
                }
                Some(Method::HEAD) => {
                    event!(Level::DEBUG, "adding path HEAD {path}");
                    head(api)
                }
                Some(Method::DELETE) => {
                    event!(Level::DEBUG, "adding path DELETE {path}");
                    delete(api)
                }
                _ => {
                    unimplemented!("Unsupported method, {:?}", self.method)
                }
            }
        }
    }

    fn response(context: &mut lifec::prelude::ThunkContext) -> Response {
        if let Some(response) = context.take_response() {
            response.into()
        } else {
            event!(Level::WARN, "Context did not generate a response, returning a 503");
            Registry::soft_fail()
        }
    }
}

/// Default registry proxy handler,
/// 
#[handler]
async fn proxy_api<R>(
    request: &poem::Request,
    body: Body,
    Path((repo, reference)): Path<(String, Option<String>)>,
    Query(ProxyRoute { ns, .. }): Query<ProxyRoute<R>>,
    resolve: Data<&ProxyRoute<R>>,
    registry: Data<&Registry>,
    context: Data<&ThunkContext>,
) -> Response 
where
    R: RouteParameters
{ 
    registry
        .proxy_request::<ProxyRoute<R>>(
            &context,
            resolve
                .operation
                .clone()
                .expect("should have an operation name"),
            request,
            Some(body.into()),
            ns,
            repo.trim_end_matches(R::ident().replace("_", "/").as_str()).trim_end_matches("/"),
            reference.filter(|r| !r.is_empty()),
        ).await
}
