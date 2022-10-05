use crate::Proxy;
use hyper::{http::StatusCode, Method};
use lifec::{AttributeIndex, ThunkContext};
use poem::{
    handler,
    web::{Data, Path, Query},
    Request, Response,
};
use serde::Deserialize;
use tracing::{event, Level};

/*
# Table of OCI distribution manifest apis

end-3	GET / HEAD	/v2/<name>/manifests/<reference>	                                        200	404
end-7	PUT	        /v2/<name>/manifests/<reference>	                                        201	404
end-9	DELETE	    /v2/<name>/manifests/<reference>	                                        202	404/400/405
*/

/// Struct for manifest api query parameters
///
#[derive(Deserialize)]
pub struct ManifestAPIParams {
    ns: String,
}
/// Resolves an image
///
#[handler]
pub async fn manifests_api(
    request: &Request,
    method: poem::http::Method,
    body: poem::Body,
    Path((name, reference)): Path<(String, String)>,
    Query(ManifestAPIParams { ns }): Query<ManifestAPIParams>,
    context: Data<&ThunkContext>,
) -> Response {
    if !context.is_enabled("proxy_enabled") {
        return Response::builder()
            .status(StatusCode::SERVICE_UNAVAILABLE)
            .finish();
    }
    let name = name.trim_end_matches("/manifests");
    event!(
        Level::DEBUG,
        "Got resolve request, repo: {name} ref: {reference} host: {ns}"
    );
    event!(Level::TRACE, "{:#?}", request);

    let mut input = context.clone();

    match method {
        Method::PUT => {
            if !body.is_empty() {
                match body.into_bytes().await.ok() {
                    Some(bytes) => {
                        input.state_mut().with_binary("body", bytes).with_symbol(
                            "content-type",
                            request.header("content-type").unwrap_or_default(),
                        );
                    }
                    None => {}
                }
            }
        }
        _ => {}
    }

    input
        .state_mut()
        .with_symbol("name", name)
        .with_symbol("repo", name)
        .with_symbol("reference", reference)
        .with_symbol("ns", &ns)
        .with_symbol("api", format!("https://{ns}/v2{}", request.uri().path()))
        .with_symbol("accept", request.header("accept").unwrap_or_default())
        .with_symbol("method", method);

    Proxy::handle(&input).await
}
