use hyper::http::StatusCode;
use lifec::{ThunkContext, AttributeIndex, Host, Executor};
use poem::{Request, web::{Path, Query, Data}, Response, handler};
use serde::Deserialize;
use tracing::{event, Level};
use crate::Proxy;

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
    input
        .state_mut()
        .with_symbol("repo", name)
        .with_symbol("reference", reference)
        .with_symbol("ns", &ns)
        .with_symbol("api", format!("https://{ns}/v2{}", request.uri().path()))
        .with_symbol("accept", request.header("accept").unwrap_or_default())
        .with_symbol("method", method);

    let mut host = Host::load_content::<Proxy>(input.state().find_text("proxy_src").unwrap());

    let input = host.execute(&input);
    Proxy::into_response(&input)
}