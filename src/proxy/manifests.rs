use super::proxy_route::RouteParameters;

/// Route plugin to handle registry manifest requests,
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
#[derive(Default, Clone)]
pub struct Manifests;

impl RouteParameters for Manifests {
    fn path() -> &'static str {
        "/:repo<[a-zA-Z0-9/_-]+(?:manifests)>/:reference"
    }

    fn ident() -> &'static str {
        "manifests"
    }
}

