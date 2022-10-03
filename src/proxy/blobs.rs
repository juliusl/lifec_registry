use crate::Proxy;
use hyper::http::StatusCode;
use lifec::{AttributeIndex, ThunkContext};
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
    Path((name, digest)): Path<(String, String)>,
    Query(BlobDownloadParams { ns }): Query<BlobDownloadParams>,
    context: Data<&ThunkContext>,
) -> Response {
    if !context.is_enabled("proxy_enabled") {
        return Proxy::soft_fail();
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

    Proxy::handle(&input).await
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

        Proxy::handle(&input).await
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

        Proxy::handle(&input).await
    } else if let None = digest {
        event!(Level::DEBUG, "Got blob_upload_session_id request, {name}");
        event!(Level::TRACE, "{:#?}", request);

        let mut input = context.clone();
        input
            .state_mut()
            .with_symbol("name", name)
            .with_symbol("api", format!("https://{ns}/v2{}", request.uri().path()));

        Proxy::handle(&input).await
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

    Proxy::handle(&input).await
}
