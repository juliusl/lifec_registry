use std::sync::Arc;

use crate::RegistryProxy;
use hyper::http::StatusCode;
use lifec::prelude::{AttributeIndex, Host, ThunkContext};
use poem::{
    handler,
    web::{Data, Path, Query},
    Request, Response,
};
use serde::Deserialize;
use tracing::event;
use tracing::Level;

/*
# Table of OCI Blob apis

ID      METHOD      PATH                                                                        RESPONSES
end-2	GET / HEAD	/v2/<name>/blobs/<digest>	                                                200	404
end-10	DELETE	    /v2/<name>/blobs/<digest>	                                                202	404/405

end-4a	POST	    /v2/<name>/blobs/uploads/	                                                202	404
end-4b	POST	    /v2/<name>/blobs/uploads/             ?digest=<digest>	                    201/202	404/400
end-11	POST	    /v2/<name>/blobs/uploads/             ?mount=<digest>&from=<other_name>	    201	404

end-5	PATCH	    /v2/<name>/blobs/uploads/<reference>	                                    202	404/416
end-6	PUT	        /v2/<name>/blobs/uploads/<reference>  ?digest=<digest>	                    201	404/400
*/

/// Struct for blob download query parameters
///
#[derive(Deserialize)]
pub struct BlobDownloadParams {
    ns: String,
}

/// API handler for end-2
///
#[handler]
pub async fn blob_download_api(
    request: &Request,
    method: poem::http::Method,
    Path((name, digest)): Path<(String, String)>,
    Query(BlobDownloadParams { ns }): Query<BlobDownloadParams>,
    context: Data<&ThunkContext>,
    host: Data<&Arc<Host>>,
) -> Response {
    if !context.is_enabled("proxy_enabled") {
        return RegistryProxy::soft_fail();
    }

    let name = name.trim_end_matches("/blobs");
    event!(Level::DEBUG, "Got download_blobs request, {name} {digest}");
    event!(Level::TRACE, "{:#?}", request);

    let mut input = context.clone();

    input
        .state_mut()
        .with_symbol("name", name)
        .with_symbol("ns", &ns)
        .with_symbol("method", &method)
        .with_symbol("api", format!("https://{ns}/v2{}", request.uri().path()))
        .with_symbol("digest", digest);

    if let Some(accept) = request.header("accept") {
        input.state_mut().add_symbol("accept", accept)
    }

    RegistryProxy::handle(&host, "blobs", method.to_string(), &input).await
}

/// API handler for end-4a, end-4b, end-11
///
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
    method: poem::http::Method,
    body: poem::Body,
    Path(name): Path<String>,
    Query(ImportParameters {
        digest,
        mount,
        from,
        ns,
    }): Query<ImportParameters>,
    context: Data<&ThunkContext>,
    host: Data<&Arc<Host>>,
) -> Response {
    if !context.is_enabled("proxy_enabled") {
        return Response::builder()
            .status(StatusCode::SERVICE_UNAVAILABLE)
            .finish();
    }

    let name = name.trim_end_matches("/blobs");
    let mut input = context.clone();

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

    if let (Some(mount), Some(from)) = (mount, from) {
        event!(
            Level::DEBUG,
            "Got blob_import request, {name}, {mount}, {from}"
        );
        event!(Level::TRACE, "{:#?}", &request);
        input
            .state_mut()
            .with_symbol("name", name)
            .with_symbol("mount", mount)
            .with_symbol("from", from)
            .with_symbol("method", method)
            .with_symbol("api", format!("https://{ns}/v2{}", request.uri().path()));

        RegistryProxy::handle(&host, "blobs", "import", &input).await
    } else if let Some(digest) = digest {
        event!(
            Level::DEBUG,
            "Got blob_upload_monolith request, {name}, {digest}"
        );
        event!(Level::TRACE, "{:#?}", request);
        input
            .state_mut()
            .with_symbol("name", name)
            .with_symbol("digest", digest)
            .with_symbol("method", method)
            .with_symbol("api", format!("https://{ns}/v2{}", request.uri().path()));

        RegistryProxy::handle(&host, "blobs", "upload_monolith", &input).await
    } else if let None = digest {
        event!(Level::DEBUG, "Got blob_upload_session_id request, {name}");
        event!(Level::TRACE, "{:#?}", request);
        input
            .state_mut()
            .with_symbol("name", name)
            .with_symbol("method", method)
            .with_symbol("api", format!("https://{ns}/v2{}", request.uri().path()));

        RegistryProxy::handle(&host, "blobs", "upload_session_id", &input).await
    } else {
        return Response::builder()
            .status(StatusCode::SERVICE_UNAVAILABLE)
            .finish();
    }
}

/// API handler for end-5, end-6
///
#[derive(Deserialize)]
pub struct UploadParameters {
    digest: Option<String>,
    ns: String,
}
#[handler]
pub async fn blob_chunk_upload_api(
    request: &Request,
    method: poem::http::Method,
    body: poem::Body,
    Path((name, reference)): Path<(String, String)>,
    Query(UploadParameters { digest, ns }): Query<UploadParameters>,
    context: Data<&ThunkContext>,
    host: Data<&Arc<Host>>,
) -> Response {
    if !context.is_enabled("proxy_enabled") {
        return Response::builder()
            .status(StatusCode::SERVICE_UNAVAILABLE)
            .finish();
    }
    let name = name.trim_end_matches("/blobs");

    let mut input = context.clone();

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

    event!(
        Level::DEBUG,
        "Got {method} blob_upload_chunks request, {name} {reference}, {:?}",
        digest
    );
    event!(Level::TRACE, "{:#?}", request);
    input
        .state_mut()
        .with_symbol("name", name)
        .with_symbol("reference", reference)
        .with_symbol("method", method.as_str().to_ascii_uppercase())
        .with_symbol("api", format!("https://{ns}/v2{}", request.uri().path()))
        .with_symbol("digest", digest.unwrap_or_default());

    RegistryProxy::handle(&host, "blobs", "upload_chunks", &input).await
}
