use lifec::{
    editor::{RuntimeEditor, Call},
    plugins::{Plugin, Project, ThunkContext, combine, Println},
    Component, DenseVecStorage, HashMapStorage, Runtime,
};
use lifec_poem::{WebApp, AppHost};
use poem::{
    get, handler,
    http::{Method, StatusCode},
    patch, post,
    web::{Data, Path, Query},
    EndpointExt, Request, Response, Route,
};
use serde::Deserialize;
use tracing::{event, Level};

use crate::{
    create_runtime, BlobImport, BlobUploadChunks, BlobUploadMonolith, BlobUploadSessionId,
    DownloadBlob, ListTags, Resolve, Upstream, Authenticate, Login, Index,
};

/// Designed to be used w/ containerd's registry config described here:
/// https://github.com/containerd/containerd/blob/main/docs/hosts.md
///
/// To enable this feature, it consists of writing a hosts.toml under /etc/containerd/certs.d/{host_name}
///
/// Here is an example to run a simple test w/ this mirror:
/// ```toml
/// server = "https://registry-1.docker.io"
///
/// [host."http://localhost:5000"]
/// capabilities = [ "resolve", "pull" ]
/// skip_verify = true
/// ```
///
/// And, then to test, you can use ctr:
/// ```sh
/// sudo ctr images pull --hosts-dir "/etc/containerd/certs.d" docker.io/library/python:latest  
/// ```
///
/// To setup the runtime, you can enable this setting in /etc/containerd/config.toml
///
/// ```toml
/// config_path = "/etc/containerd/certs.d"
/// ```
///
#[derive(Component, Clone, Default)]
#[storage(HashMapStorage)]
pub struct Mirror<Event>(ThunkContext, Event)
where
    Event: MirrorEvent + Default + Send + Sync + 'static;

/// Wrapper around mirror event actions
/// 
#[derive(Clone)]
pub struct MirrorAction {
    on_response: fn(tc: &ThunkContext) -> Response,
    on_error: fn(err: String, tc: &ThunkContext) -> Response,
}

/// Plugin to host the mirror
///
#[derive(Component, Default, Clone)]
#[storage(DenseVecStorage)]
pub struct MirrorHost<Event>(Event)
where
    Event: MirrorEvent + Default + Send + Sync + 'static;

/// Event handlers for after a mirror plugin completes
/// 
/// Can be implemented on a per-feature basis to extend the registry on the client side
///
pub trait MirrorEvent
{
    /// Called after the plugin finishes, and if the plugin returned the next thunk_context
    /// 
    fn resolve_response(tc: &ThunkContext) -> Response;

    /// Called after the plugin finishes, and if the plugin task returned an error
    /// 
    fn resolve_error(err: String, tc: &ThunkContext) -> Response;
}

impl MirrorAction {
    fn from<Event>() -> Self
    where
        Event: MirrorEvent + Default + Send + Sync + 'static,
    {
        MirrorAction {
            on_response: Event::resolve_response,
            on_error: Event::resolve_error,
        }
    }

    fn handle_response(&self, tc: &ThunkContext) -> Response {
        (self.on_response)(tc)
    }

    fn handle_error(&self, err: String, tc: &ThunkContext) -> Response {
        (self.on_error)(err, tc)
    }

    async fn handle<P>(&self, tc: &mut ThunkContext) -> Response
    where
        P: Plugin<ThunkContext>,
    {
        tc.as_mut().with_text("thunk_symbol", P::symbol());

        if let Some((task, _cancel)) = P::call_with_context(tc) {
            match task.await {
                Ok(result) => self.handle_response(&result),
                Err(err) => self.handle_error(format!("{}", err), &tc.clone()),
            }
        } else {
            soft_fail()
        }
    }
}

/// Fails in a way that the runtime will fallback to the upstream server
fn soft_fail() -> Response {
    Response::builder()
        .status(StatusCode::SERVICE_UNAVAILABLE)
        .finish()
}

/// Fails in a way that stops the runtime from completing it's action
fn blocking_fail() -> Response {
    Response::builder().finish()
}

impl<Event> Plugin<ThunkContext> for MirrorHost<Event>
where
    Event: MirrorEvent + Default + Send + Sync + 'static,
{
    fn symbol() -> &'static str {
        "mirror_host"
    }

    fn description() -> &'static str {
r#"
Hosts the mirror server locally, using lifec_poem's app_host plugin.
TLS Settings will be used if present.
"#
    }

    fn call_with_context(context: &mut ThunkContext) -> Option<lifec::plugins::AsyncContext> {
        combine::<AppHost<Mirror<Event>>, Println>()(context)
    }
}

impl<Event> Plugin<ThunkContext> for Mirror<Event>
where
    Event: MirrorEvent + Default + Send + Sync + 'static,
{
    fn symbol() -> &'static str {
        "mirror"
    }

    fn description() -> &'static str {
        "Hosts a registry mirror, to extend registry capabilities at runtime"
    }

    fn caveats() -> &'static str {
        r#"
hosts.toml must have already been installed on the machine

Design of containerd registry mirror feature
1. Add config to /etc/containerd/certs.d/{host_name}/hosts.toml
2. Content of hosts.toml
    server = "{host_name}" 

    [host."https://{address}"]
      capabilities = ["pull", "resolve"]
      ca = "path/to/{address}.crt"
"#
    }

    fn call_with_context(context: &mut ThunkContext) -> Option<lifec::plugins::AsyncContext> {
        context.clone().task(|cancel_source| {
            let tc = context.clone();
            async move {
                if let Some(project) = tc
                    .as_ref()
                    .find_text("project_src")
                    .and_then(|src| Project::load_file(src))
                {
                    let block_name = tc.block.block_name.to_string();
                    if let Some(address) = tc.as_ref().find_text("address") {
                        let project = project.with_block(&block_name, "app_host", |c| {
                            c.add_text_attr("address", &address);
                        });

                        let link = format!("https://{address}/v2");
                        let log = format!("Starting registry mirror on {link}");

                        tc.update_status_only(&log).await;
                        eprintln!("{log}");

                        let runtime = create_runtime::<Event>(project);
                        let runtime_editor = RuntimeEditor::new(runtime);
                        // tc.as_mut().add_bool_attr("proxy_dispatcher", true);

                        let mut extension = Upstream::<Event>::from(runtime_editor);
                        Runtime::start_with::<Upstream<Event>, Call>(
                            &mut extension,
                            Mirror::<Event>::symbol().to_string(),
                            &tc,
                            cancel_source,
                        );
                    }
                }

                Some(tc)
            }
        })
    }
}

impl<Event> WebApp for Mirror<Event>
where
    Event: MirrorEvent + Default + Send + Sync + 'static,
{
    fn create(context: &mut ThunkContext) -> Self {
        Self(context.clone(), Event::default())
    }

    fn routes(&mut self) -> Route {
        let context = &self.0;
        Route::new().nest(
            "/v2",
            Route::new()
                .at("/", 
                    get(index
                        .data(context.clone())
                        .data(MirrorAction::from::<Event>()))
                    .head(index
                        .data(context.clone())
                        .data(MirrorAction::from::<Event>()))
                )
                .at(
                    "/:name<[a-zA-Z0-9/_-]+(:?blobs)>/:digest",
                    get(download_blob
                        .data(context.clone())
                        .data(MirrorAction::from::<Event>())),
                )
                .at(
                    "/:name<[a-zA-Z0-9/_-]+(:?blobs)>/uploads",
                    post(blob_upload
                        .data(context.clone()))
                        .data(MirrorAction::from::<Event>()),
                )
                .at(
                    "/:name<[a-zA-Z0-9/_-]+(:?blobs)>/uploads/:reference",
                    patch(
                        blob_upload_chunks
                            .data(context.clone())
                            .data(MirrorAction::from::<Event>()),
                    )
                    .put(
                        blob_upload_chunks
                            .data(context.clone())
                            .data(MirrorAction::from::<Event>()),
                    ),
                )
                .at(
                    r#"/:name<[a-zA-Z0-9/_-]+(:?manifests)>/:reference"#,
                    get(resolve
                        .data(context.clone())
                        .data(MirrorAction::from::<Event>()))
                    .head(
                        resolve
                            .data(context.clone())
                            .data(MirrorAction::from::<Event>()),
                    )
                    .put(
                        resolve
                            .data(context.clone())
                            .data(MirrorAction::from::<Event>()),
                    )
                    .delete(resolve.data(context.clone()))
                    .data(MirrorAction::from::<Event>()),
                )
                .at(
                    ":name<[a-zA-Z0-9/_-]+(:?tags)>/list",
                    get(list_tags.data(context.clone())).data(MirrorAction::from::<Event>()),
                ),
        )
    }
}

#[handler]
async fn index(request: &Request,
    dispatcher: Data<&ThunkContext>,
    mirror_action: Data<&MirrorAction>) -> Response {
    event!(Level::DEBUG, "Got /v2 request");
    event!(Level::TRACE, "{:#?}", request);

    mirror_action.handle::<Index>(&mut dispatcher.clone()).await
}

#[derive(Deserialize)]
struct ResolveParams {
    ns: String,
}

/// Resolves an image
#[handler]
async fn resolve(
    request: &Request,
    Path((name, reference)): Path<(String, String)>,
    Query(ResolveParams { ns }): Query<ResolveParams>,
    dispatcher: Data<&ThunkContext>,
    mirror_action: Data<&MirrorAction>,
) -> Response {
    let name = name.trim_end_matches("/manifests");

    event!(
        Level::DEBUG,
        "Got resolve request, repo: {name} ref: {reference} host: {ns}"
    );
    event!(
        Level::TRACE, 
        "{:#?}", request
    );

    let mut input = dispatcher.clone();
    input.as_mut()
        .with_text("repo", name)
        .with_text("reference", reference)
        .with_text("ns", &ns)
        .with_text("api", format!("https://{ns}/v2{}", request.uri().path()))
        .add_text_attr("accept", request.header("accept").unwrap_or_default());

    mirror_action.handle::<((Login, Authenticate), Resolve)>(&mut input.clone()).await
}

#[handler]
async fn list_tags(
    request: &Request,
    Path(name): Path<String>,
    dispatcher: Data<&ThunkContext>,
    mirror_action: Data<&MirrorAction>,
) -> Response {
    let name = name.trim_end_matches("/tags");

    event!(Level::DEBUG, "Got list_tags request, {name}");
    event!(Level::TRACE, "{:#?}", request);

    let mut input = dispatcher.clone();
    input.as_mut().with_text("name", name);

    mirror_action.handle::<ListTags>(&mut input).await
}

#[handler]
async fn download_blob(
    request: &Request,
    Path((name, digest)): Path<(String, String)>,
    Query(ResolveParams { ns }): Query<ResolveParams>,
    dispatcher: Data<&ThunkContext>,
    mirror_action: Data<&MirrorAction>,
) -> Response {
    let name = name.trim_end_matches("/blobs");
    event!(Level::DEBUG, "Got download_blobs request, {name} {digest}");
    event!(Level::TRACE, "{:#?}", request);

    let mut input = dispatcher.clone();
    input.as_mut()
        .with_text("name", name)
        .with_text("ns", &ns)
        .with_text("api", format!("https://{ns}/v2{}", request.uri().path()))
        .with_text("digest", digest);
    
    if let Some(accept) = request.header("accept") {
        input.as_mut().add_text_attr("accept", accept)
    }

    mirror_action.handle::<((Login, Authenticate), DownloadBlob)>(&mut input).await
}

#[derive(Deserialize)]
struct UploadParameters {
    digest: Option<String>,
}

#[handler]
async fn blob_upload_chunks(
    request: &Request,
    method: Method,
    Path((name, reference)): Path<(String, String)>,
    Query(UploadParameters { digest }): Query<UploadParameters>,
    dispatcher: Data<&ThunkContext>,
    mirror_action: Data<&MirrorAction>,
) -> Response {
    let name = name.trim_end_matches("/blobs");

    event!(
        Level::DEBUG,
        "Got {method} blob_upload_chunks request, {name} {reference}, {:?}",
        digest
    );
    event!(Level::TRACE, "{:#?}", request);

    let mut input = dispatcher.clone();
    input.as_mut().with_text("name", name);
    input.as_mut().with_text("reference", reference);
    input.as_mut().with_text("digest", digest.unwrap_or_default());

    mirror_action.handle::<BlobUploadChunks>(&mut input).await
}

#[derive(Deserialize)]
struct ImportParameters {
    digest: Option<String>,
    mount: Option<String>,
    from: Option<String>,
}
#[handler]
async fn blob_upload(
    request: &Request,
    Path(name): Path<String>,
    Query(ImportParameters {
        digest,
        mount,
        from,
    }): Query<ImportParameters>,
    dispatcher: Data<&ThunkContext>,
    mirror_action: Data<&MirrorAction>,
) -> Response {
    let name = name.trim_end_matches("/blobs");

    if let (Some(mount), Some(from)) = (mount, from) {
        event!(
            Level::DEBUG,
            "Got blob_import request, {name}, {mount}, {from}"
        );
        event!(Level::TRACE, "{:#?}", request);


        let mut input = dispatcher.clone();
        input.as_mut().with_text("name", name);
        input.as_mut().with_text("mount", mount);
        input.as_mut().with_text("from", from);

        mirror_action.handle::<BlobImport>(&mut input).await
    } else if let Some(digest) = digest {
        event!(
            Level::DEBUG,
            "Got blob_upload_monolith request, {name}, {digest}"
        );
        event!(Level::TRACE, "{:#?}", request);

        let mut input = dispatcher.clone();
        input.as_mut().with_text("name", name);
        input.as_mut().with_text("digest", digest);

        mirror_action.handle::<BlobUploadMonolith>(&mut input).await

    } else if let None = digest {
        event!(Level::DEBUG, "Got blob_upload_session_id request, {name}");
        event!(Level::TRACE, "{:#?}", request);

        let mut input = dispatcher.clone();
        input.as_mut().with_text("name", name);

        mirror_action.handle::<BlobUploadSessionId>(&mut input).await
    } else {
        soft_fail()
    }
}

// Table of OCI Endpoints

// ID	Method	API Endpoint	Success	Failure
// end-1	GET	/v2/	                                                                            200	404/401

// end-2	GET / HEAD	/v2/<name>/blobs/<digest>	                                                200	404
// end-10	DELETE	    /v2/<name>/blobs/<digest>	                                                202	404/405

// end-4a	POST	    /v2/<name>/blobs/uploads/	                                                202	404
// end-4b	POST	    /v2/<name>/blobs/uploads/             ?digest=<digest>	                    201/202	404/400
// end-11	POST	    /v2/<name>/blobs/uploads/             ?mount=<digest>&from=<other_name>	    201	404

// end-5	PATCH	    /v2/<name>/blobs/uploads/<reference>	                                    202	404/416
// end-6	PUT	        /v2/<name>/blobs/uploads/<reference>  ?digest=<digest>	                    201	404/400

// end-8a	GET	        /v2/<name>/tags/list	                                                    200	404
// end-8b	GET	        /v2/<name>/tags/list                  ?n=<integer>&last=<integer>	        200	404

// end-3	GET / HEAD	/v2/<name>/manifests/<reference>	                                        200	404
// end-7	PUT	        /v2/<name>/manifests/<reference>	                                        201	404
// end-9	DELETE	    /v2/<name>/manifests/<reference>	                                        202	404/400/405

#[derive(Default)]
struct TestMirrorEvent;

impl Plugin<ThunkContext> for TestMirrorEvent {
    fn symbol() -> &'static str {
        "test_mirror_event"
    }

    fn call_with_context(context: &mut ThunkContext) -> Option<lifec::plugins::AsyncContext> {
        todo!()
    }
}

impl MirrorEvent for TestMirrorEvent {
    fn resolve_response(tc: &ThunkContext) -> Response {
        todo!()
    }

    fn resolve_error(err: String, tc: &ThunkContext) -> Response {
        todo!()
    }
}

#[test]
fn test_mirror() {
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let app = Mirror::<TestMirrorEvent>::default().routes();
        let cli = poem::test::TestClient::new(app);

        let resp = cli.get("/").send().await;
        resp.assert_status_is_ok();

        let resp = cli.head("/").send().await;
        resp.assert_status_is_ok();

        let resp = cli.get("/v2").send().await;
        resp.assert_status_is_ok();

        let resp = cli.get("/v2/").send().await;
        resp.assert_status_is_ok();

        let resp = cli.head("/v2").send().await;
        resp.assert_status_is_ok();

        let resp = cli.head("/v2/").send().await;
        resp.assert_status_is_ok();

        let resp = cli.get("/v2/library/test/manifests/test_ref").send().await;
        resp.assert_status_is_ok();

        let resp = cli.head("/v2/library/test/manifests/test_ref").send().await;
        resp.assert_status_is_ok();

        let resp = cli.put("/v2/library/test/manifests/test_ref").send().await;
        resp.assert_status_is_ok();

        let resp = cli
            .delete("/v2/library/test/manifests/test_ref")
            .send()
            .await;
        resp.assert_status_is_ok();

        let resp = cli.get("/v2/library/test/blobs/test_digest").send().await;
        resp.assert_status_is_ok();

        let resp = cli.post("/v2/library/test/blobs/uploads").send().await;
        resp.assert_status_is_ok();

        let resp = cli
            .patch("/v2/library/test/blobs/uploads/test")
            .send()
            .await;
        resp.assert_status_is_ok();

        let resp = cli.put("/v2/library/test/blobs/uploads/test").send().await;
        resp.assert_status_is_ok();

        let resp = cli.get("/v2/library/test/tags/list").send().await;
        resp.assert_status_is_ok();
    });
}
