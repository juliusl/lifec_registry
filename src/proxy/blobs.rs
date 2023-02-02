use super::proxy_route::RouteParameters;

/// Route plugin to handle registry download blob requests,
///
/// Example:
/// : .mirror     <azurecr.io>
/// : .host       <address> resolve, pull
///
/// + .proxy      <address>
/// : .blobs  
/// : .get        <operation-name>
/// : .head       <operation-name>
///
#[derive(Default, Clone)]
pub struct Blobs;

impl RouteParameters for Blobs {
    fn path() -> &'static str {
        "/:repo<[a-zA-Z0-9/_-]+(?:blobs)>/:reference"
    }

    fn ident() -> &'static str {
        "blobs"
    }
}