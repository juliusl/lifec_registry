use lifec::{ThunkContext, AttributeIndex, Host, Executor};
use crate::Proxy;
use poem::{Request, Response, web::{Query, Path, Data}, handler};
use serde::Deserialize;
use hyper::http::StatusCode;
use tracing::event;
use tracing::Level;

#[derive(Deserialize)]
pub struct ManifestAPIParams {
    ns: String,
}

#[handler]
pub async fn blob_download_api(
    request: &Request,
    Path((name, digest)): Path<(String, String)>,
    Query(ManifestAPIParams { ns }): Query<ManifestAPIParams>,
    context: Data<&ThunkContext>,
) -> Response {
    if !context.is_enabled("proxy_enabled") {
        return Response::builder()
            .status(StatusCode::SERVICE_UNAVAILABLE)
            .finish();
    }

    let name = name.trim_end_matches("/blobs");
    event!(Level::DEBUG, "Got download_blobs request, {name} {digest}");
    event!(Level::TRACE, "{:#?}", request);

    let mut input = context.clone();
    input
        .state_mut()
        .with_symbol("name", name)
        .with_symbol("ns", &ns)
        .with_symbol("api", format!("https://{ns}/v2{}", request.uri().path()))
        .with_symbol("digest", digest);

    if let Some(accept) = request.header("accept") {
        input.state_mut().add_text_attr("accept", accept)
    }

    let mut host = Host::load_content::<Proxy>(input.state().find_text("proxy_src").unwrap());

    let input = host.execute(&input);
    Proxy::into_response(&input)
}

#[derive(Deserialize)]
pub struct UploadParameters {
    digest: Option<String>,
    ns: String,
}
#[handler]
pub async fn blob_chunk_upload_api(
    request: &Request,
    method: poem::http::Method,
    Path((name, reference)): Path<(String, String)>,
    Query(UploadParameters { digest, ns }): Query<UploadParameters>,
    context: Data<&ThunkContext>,
) -> Response {
    if !context.is_enabled("proxy_enabled") {
        return Response::builder()
            .status(StatusCode::SERVICE_UNAVAILABLE)
            .finish();
    }

    let name = name.trim_end_matches("/blobs");

    event!(
        Level::DEBUG,
        "Got {method} blob_upload_chunks request, {name} {reference}, {:?}",
        digest
    );
    event!(Level::TRACE, "{:#?}", request);

    let mut input = context.clone();
    input
        .state_mut()
        .with_symbol("name", name)
        .with_symbol("reference", reference)
        .with_symbol("api", format!("https://{ns}/v2{}", request.uri().path()))
        .with_symbol("digest", digest.unwrap_or_default());

    let mut host = Host::load_content::<Proxy>(input.state().find_text("proxy_src").unwrap());

    let input = host.execute(&input);
    Proxy::into_response(&input)
}

#[derive(Deserialize)]
pub struct ImportParameters {
    digest: Option<String>,
    mount: Option<String>,
    from: Option<String>,
    ns: String,
}
#[handler]
pub async fn blob_upload_api(
    request: &Request,
    Path(name): Path<String>,
    Query(ImportParameters {
        digest,
        mount,
        from,
        ns,
    }): Query<ImportParameters>,
    context: Data<&ThunkContext>,
) -> Response {
    if !context.is_enabled("proxy_enabled") {
        return Response::builder()
            .status(StatusCode::SERVICE_UNAVAILABLE)
            .finish();
    }

    let name = name.trim_end_matches("/blobs");

    if let (Some(mount), Some(from)) = (mount, from) {
        event!(
            Level::DEBUG,
            "Got blob_import request, {name}, {mount}, {from}"
        );
        event!(Level::TRACE, "{:#?}", request);

        let mut input = context.clone();
        input
            .state_mut()
            .with_symbol("name", name)
            .with_symbol("mount", mount)
            .with_symbol("from", from)
            .with_symbol("api", format!("https://{ns}/v2{}", request.uri().path()));

        let mut host = Host::load_content::<Proxy>(input.state().find_text("proxy_src").unwrap());

        let input = host.execute(&input);
        Proxy::into_response(&input)
    } else if let Some(digest) = digest {
        event!(
            Level::DEBUG,
            "Got blob_upload_monolith request, {name}, {digest}"
        );
        event!(Level::TRACE, "{:#?}", request);

        let mut input = context.clone();
        input
            .state_mut()
            .with_symbol("name", name)
            .with_symbol("digest", digest)
            .with_symbol("api", format!("https://{ns}/v2{}", request.uri().path()));

        let mut host = Host::load_content::<Proxy>(input.state().find_text("proxy_src").unwrap());

        let input = host.execute(&input);
        Proxy::into_response(&input)
    } else if let None = digest {
        event!(Level::DEBUG, "Got blob_upload_session_id request, {name}");
        event!(Level::TRACE, "{:#?}", request);

        let mut input = context.clone();
        input
            .state_mut()
            .with_symbol("name", name)
            .with_symbol("api", format!("https://{ns}/v2{}", request.uri().path()));

        let mut host = Host::load_content::<Proxy>(input.state().find_text("proxy_src").unwrap());

        let input = host.execute(&input);
        Proxy::into_response(&input)
    } else {
        return Response::builder()
            .status(StatusCode::SERVICE_UNAVAILABLE)
            .finish();
    }
}