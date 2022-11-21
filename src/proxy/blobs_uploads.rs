use std::sync::Arc;

use hyper::Method;
use lifec::prelude::{AttributeParser, Host, SpecialAttribute, Value, ThunkContext};
use lifec_poem::RoutePlugin;
use poem::{
    handler,
    web::{Data, Path, Query},
    EndpointExt, Response, RouteMethod, post,
};
use serde::{Deserialize, Serialize};
use specs::{Component, VecStorage, WorldExt};

use crate::Registry;

/// Route plugin to handle registry blob uploads,
///
/// Example:
/// : .mirror     <azurecr.io>
/// : .host       <address> resolve, push
///
/// + .proxy      <address>
/// : .blobs_uploads
/// : .post        <operation-name>
///
#[derive(Component, Default, Clone, Serialize, Deserialize)]
#[storage(VecStorage)]
pub struct BlobsUploads {
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

impl BlobsUploads {
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

impl SpecialAttribute for BlobsUploads {
    fn ident() -> &'static str {
        "blobs_uploads"
    }

    fn parse(parser: &mut AttributeParser, content: impl AsRef<str>) {
        let world = parser.world().expect("should have a world");

        let blobs = BlobsUploads::default();

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

        parser.add_custom_with("post", |p, c| {
            let last_entity = p.last_child_entity().expect("should have an entity");
            let world = p.world().expect("should have a world");

            let blobs = {
                let blobs = world.read_component::<BlobsUploads>();
                let blobs = blobs.get(last_entity).expect("should have a blob");
                blobs.clone()
            };

            let mut route = blobs.clone();
            route.method = Some(Method::POST);
            route.operation = Some(c);
            let route_entity = world.entities().create();
            world
                .write_component()
                .insert(route_entity, route)
                .expect("should be able to insert component");
        });
    }
}

impl RoutePlugin for BlobsUploads {
    fn route(&self,  _: Option<RouteMethod>) -> RouteMethod {
        match self.method {
            Some(Method::POST) => {
                post(blobs_uploads_api.data(self.clone()).data(self.host.clone()).data(self.context.clone()))
            }
            _ => {
                panic!("Unsupported method, {:?}", self.method)
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
async fn blobs_uploads_api(
    request: &poem::Request,
    Path(repo): Path<String>,
    Query(BlobsUploads { ns, .. }): Query<BlobsUploads>,
    blobs_uploads: Data<&BlobsUploads>,
    host: Data<&Arc<Host>>,
    context: Data<&ThunkContext>,
) -> Response {
    let mut registry = host.world().system_data::<Registry>();

    registry
        .proxy_request::<BlobsUploads>(
            &context,
            blobs_uploads
                .operation
                .clone()
                .expect("should have an operation name"),
            request,
            None,
            ns,
            repo,
            "",
        ).await
}
