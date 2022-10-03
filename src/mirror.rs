use std::{path::PathBuf, str::FromStr};

use hyper::Uri;
use lifec::{
    plugins::{Plugin, ThunkContext},
    AttributeIndex, BlockObject, BlockProperties, Component, CustomAttribute, HashMapStorage,
    Interpreter, Value, Operation,
};
use lifec_poem::{AppHost, WebApp};
use logos::Logos;
use poem::{
    get, handler,
    http::{Method, StatusCode},
    patch, post,
    web::{Data, Path, Query},
    EndpointExt, Request, Response, Route,
};
use serde::Deserialize;
use toml::value::Map;
use tracing::{event, Level};

use crate::{
    mirror::mirror_action::soft_fail, Authenticate, BlobImport, BlobUploadChunks,
    BlobUploadMonolith, BlobUploadSessionId, DownloadBlob, Index, ListTags, Login, Resolve, Proxy,
};

mod mirror_action;
use mirror_action::MirrorAction;

mod mirror_proxy;
pub use mirror_proxy::MirrorProxy;

mod host_capabilities;
use host_capabilities::HostCapability;

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
pub struct Mirror<M>
where
    M: MirrorProxy + Default + Send + Sync + 'static,
{
    _proxy: M,
    context: ThunkContext,
}

impl<M> Mirror<M>
where
    M: MirrorProxy + Default + Send + Sync + 'static,
{
    /// Ensures the hosts dir exists
    ///
    async fn ensure_hosts_dir(app_host: impl AsRef<str>) {
        let hosts_dir = format!("/etc/containerd/certs.d/{}/", app_host.as_ref());

        let path = PathBuf::from(hosts_dir);
        if !path.exists() {
            event!(
                Level::DEBUG,
                "hosts directory did not exist, creating {:?}",
                &path
            );
            match tokio::fs::create_dir_all(&path).await {
                Ok(_) => {
                    event!(Level::DEBUG, "Created hosts directory");
                }
                Err(err) => {
                    event!(Level::ERROR, "Could not create directories {err}");
                }
            }
        }

        let path = path.join("hosts.toml");
        if !path.exists() {
            let output_hosts_toml = PathBuf::from(format!(
                ".work/etc/containerd/certs.d/{}/hosts.toml",
                app_host.as_ref()
            ));
            event!(
                Level::DEBUG,
                "hosts.toml did not exist, creating {:?}",
                &path
            );

            assert!(
                output_hosts_toml.exists(),
                "should have been created before this plugin runs"
            );

            match tokio::fs::copy(output_hosts_toml, &path).await {
                Ok(_) => {
                    event!(Level::INFO, "Copied hosts.toml tp {:?}", path);
                }
                Err(err) => {
                    panic!("Could not copy hosts.toml, {err}");
                }
            }
        }
    }
}

impl<M> Plugin for Mirror<M>
where
    M: MirrorProxy + Default + Send + Sync + 'static,
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

    fn call(context: &ThunkContext) -> Option<lifec::plugins::AsyncContext> {
        context.task(|_| {
            let tc = context.clone();
            async move {
                // if !tc.is_enabled("skip_hosts_dir_check") {
                //     let app_host = tc
                //         .state()
                //         .find_symbol("mirror")
                //         .expect("host name to mirror is required");

                //     Self::ensure_hosts_dir(app_host).await;
                // }

                match AppHost::<Proxy>::call(&tc) {
                    Some((task, _)) => match task.await {
                        Ok(tc) => {
                            event!(Level::INFO, "Exiting");
                            Some(tc)
                        }
                        Err(err) => {
                            event!(Level::ERROR, "Error from app_host {err}");
                            None
                        }
                    },
                    _ => None,
                }
            }
        })
    }

    /// This will add some custom attributes to the parser for handling environment setup,
    ///
    /// # Usage Example
    ///
    /// ```runmd
    /// ``` test containerd
    /// + .runtime
    /// : .mirror   azurecr.io
    /// : .server   https://example.azurecr.io
    /// : .host     localhost:5000, pull, resolve, push
    /// : .https    hosts.crt
    /// ```
    ///
    fn compile(parser: &mut lifec::AttributeParser) {
        // This attribute handles setting the
        parser.add_custom(CustomAttribute::new_with(
            "server",
            |p, content| match Uri::from_str(&content) {
                Ok(upstream) => {
                    let last = p.last_child_entity().expect("child required to edit");
                    p.define_child(last, "server", Value::Symbol(upstream.to_string()));
                }
                Err(err) => {
                    event!(Level::ERROR, "Could not parse uri {}, {err}", content);
                }
            },
        ));

        parser.add_custom(CustomAttribute::new_with("host", |p, content| {
            let args = content.split_once(",");

            if let Some((proxy_to, capabilities)) = args {
                let last = p
                    .last_child_entity()
                    .expect("child entity required to edit");
                p.define_child(last, "app_host", Value::Symbol(proxy_to.to_string()));

                let mut lexer = HostCapability::lexer(capabilities);
                let feature_name = format!("feature_{}", proxy_to);
                while let Some(feature) = lexer.next() {
                    match feature {
                        HostCapability::Resolve => {
                            p.define_child(
                                last,
                                &feature_name,
                                Value::Symbol("resolve".to_string()),
                            );
                        }
                        HostCapability::Push => {
                            p.define_child(last, &feature_name, Value::Symbol("push".to_string()));
                        }
                        HostCapability::Pull => {
                            p.define_child(last, &feature_name, Value::Symbol("pull".to_string()));
                        }
                        HostCapability::Error => continue,
                    }
                }
            }
        }));

        parser.add_custom(CustomAttribute::new_with("https", |p, content| {
            let path = PathBuf::from(content);
            let path = path.canonicalize().expect("must exist");
            let last = p.last_child_entity().expect("child entity required");
            p.define_child(last, "https", Value::Symbol(format!("{:?}", path)));
        }));
    }
}

impl<M> Interpreter for Mirror<M>
where
    M: MirrorProxy + Default + Send + Sync + 'static,
{
    fn initialize(&self, _world: &mut lifec::World) {
        // TODO
    }

    fn interpret(&self, _world: &lifec::World, block: &lifec::Block) {
        // Only interpret blocks with mirror symbol
        if block.symbol() == "mirror" && !block.name().is_empty() {
            let output_dir = PathBuf::from(".work/etc/containerd/certs.d");
            for i in block
                .index()
                .iter()
                .filter(|i| i.root().name() == "runtime")
            {
                /*
                Generate hosts.toml files for all mirrors found in state
                Example hosts.toml -
                ```toml
                server = "https://registry-1.docker.io"

                [host."http://192.168.31.250:5000"]
                capabilities = ["pull", "resolve", "push"]
                skip_verify = true
                ```
                */
                for (_, properties) in i
                    .iter_children()
                    .filter(|c| c.1.property("mirror").is_some())
                {
                    let host_name = properties
                        .property("mirror")
                        .and_then(|p| p.symbol())
                        .expect("host name is required");

                    let mut hosts_config = Map::new();

                    let app_hosts = properties
                        .property("app_host")
                        .and_then(|p| p.symbol_vec())
                        .expect("app_host is required for mirror");

                    for app_host in app_hosts {
                        let feature_name = format!("feature_{}", app_host);
                        let features = properties
                            .property(feature_name)
                            .and_then(|p| p.symbol_vec())
                            .unwrap_or(vec![]);
                        let mut host_settings = Map::new();
                        let features = toml::Value::Array(
                            features
                                .iter()
                                .map(|f| toml::Value::String(f.to_string()))
                                .collect::<Vec<_>>(),
                        );
                        host_settings.insert("capabilities".to_string(), features);
                        let https = properties.property("https").and_then(|p| p.symbol());
                        if let Some(https) = https {
                            let host_key = format!(r#"host."https://{}""#, app_host);
                            host_settings
                                .insert("ca".to_string(), toml::Value::String(https.to_string()));
                            hosts_config.insert(host_key, toml::Value::Table(host_settings));
                        } else {
                            let host_key = format!(r#"host."http://{}""#, app_host);
                            host_settings
                                .insert("skip_verify".to_string(), toml::Value::Boolean(true));
                            hosts_config.insert(host_key, toml::Value::Table(host_settings));
                        }
                    }

                    let output_dir = output_dir.join(host_name);
                    std::fs::create_dir_all(&output_dir).expect("should be able to create dirs");

                    let mut content = toml::ser::to_string(&hosts_config)
                        .expect("should serialize")
                        .lines()
                        .map(|l| {
                            if l.trim().starts_with("[") {
                                l.replace(r#"[""#, "[")
                                    .replace(r#"\""#, r#"""#)
                                    .replace(r#""]"#, "]")
                            } else {
                                l.to_string()
                            }
                        })
                        .collect::<Vec<_>>();

                    let server = properties.property("server").and_then(|p| p.symbol());
                    if let Some(server) = server {
                        content.insert(0, format!(r#"server = "{server}""#));
                        content.insert(1, String::default());
                    }

                    std::fs::write(output_dir.join("hosts.toml"), content.join("\n"))
                        .expect("should be able to write");
                }
            }
        }
    }
}

impl<Event> BlockObject for Mirror<Event>
where
    Event: MirrorProxy + Default + Send + Sync + 'static,
{
    fn query(&self) -> BlockProperties {
        BlockProperties::default().require("mirror")
    }

    fn parser(&self) -> Option<lifec::CustomAttribute> {
        Some(Self::as_custom_attr())
    }
}

impl<P> WebApp for Mirror<P>
where
    P: MirrorProxy + Default + Send + Sync + 'static,
{
    fn create(context: &mut ThunkContext) -> Self {
        Self {
            context: context.clone(),
            _proxy: P::default(),
        }
    }

    fn routes(&mut self) -> Route {
        let context = &self.context;
        Route::new().nest(
            "/v2",
            Route::new()
                .at(
                    "/",
                    get(index.data(context.clone()).data(MirrorAction::from::<P>()))
                        .head(index.data(context.clone()).data(MirrorAction::from::<P>())),
                )
                .at(
                    "/:name<[a-zA-Z0-9/_-]+(?:blobs)>/:digest",
                    get(download_blob
                        .data(context.clone())
                        .data(MirrorAction::from::<P>())),
                )
                .at(
                    "/:name<[a-zA-Z0-9/_-]+(?:blobs)>/uploads",
                    post(
                        blob_upload
                            .data(context.clone())
                            .data(MirrorAction::from::<P>()),
                    ),
                )
                .at(
                    "/:name<[a-zA-Z0-9/_-]+(?:blobs)>/uploads/:reference",
                    patch(
                        blob_upload_chunks
                            .data(context.clone())
                            .data(MirrorAction::from::<P>()),
                    )
                    .put(
                        blob_upload_chunks
                            .data(context.clone())
                            .data(MirrorAction::from::<P>()),
                    ),
                )
                .at(
                    "/:name<[a-zA-Z0-9/_-]+(?:manifests)>/:reference",
                    get(resolve
                        .data(context.clone())
                        .data(MirrorAction::from::<P>()))
                    .head(
                        resolve
                            .data(context.clone())
                            .data(MirrorAction::from::<P>()),
                    )
                    .put(
                        resolve
                            .data(context.clone())
                            .data(MirrorAction::from::<P>()),
                    )
                    .delete(
                        resolve
                            .data(context.clone())
                            .data(MirrorAction::from::<P>()),
                    ),
                )
                .at(
                    "/:name<[a-zA-Z0-9/_-]+(?:tags)>/list",
                    get(list_tags
                        .data(context.clone())
                        .data(MirrorAction::from::<P>())),
                ),
        )
    }
}

#[derive(Deserialize)]
struct IndexParams {
    ns: Option<String>,
}
#[handler]
async fn index(
    request: &Request,
    Query(IndexParams { ns }): Query<IndexParams>,
    dispatcher: Data<&ThunkContext>,
    mirror_action: Data<&MirrorAction>,
) -> Response {
    event!(Level::DEBUG, "Got /v2 request");
    event!(Level::TRACE, "{:#?}", request);

    let mut input = dispatcher.clone();

    if let Some(ns) = ns {
        input.state_mut().with_symbol("ns", &ns);
    }

    if let Some(response) = mirror_action.proxy(&mut input, request) {
        response
    } else {
        mirror_action.handle::<Index>(&mut input).await
    }
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
    event!(Level::TRACE, "{:#?}", request);

    let mut input = dispatcher.clone();
    input
        .state_mut()
        .with_symbol("repo", name)
        .with_symbol("reference", reference)
        .with_symbol("ns", &ns)
        .with_symbol("api", format!("https://{ns}/v2{}", request.uri().path()))
        .add_symbol("accept", request.header("accept").unwrap_or_default());

    if let Some(response) = mirror_action.proxy(&mut input, request) {
        response
    } else {
        mirror_action
            .handle::<((Login, Authenticate), Resolve)>(&mut input.clone())
            .await
    }
}

#[derive(Deserialize)]
struct ListTagsParams {
    ns: String,
}
#[handler]
async fn list_tags(
    request: &Request,
    Path(name): Path<String>,
    Query(ListTagsParams { ns }): Query<ListTagsParams>,
    dispatcher: Data<&ThunkContext>,
    mirror_action: Data<&MirrorAction>,
) -> Response {
    let name = name.trim_end_matches("/tags");

    event!(Level::DEBUG, "Got list_tags request, {name}");
    event!(Level::TRACE, "{:#?}", request);

    let mut input = dispatcher.clone();
    input
        .state_mut()
        .with_symbol("ns", ns)
        .with_symbol("name", name);

    if let Some(response) = mirror_action.proxy(&mut input, request) {
        response
    } else {
        mirror_action
            .handle::<((Login, Authenticate), ListTags)>(&mut input)
            .await
    }
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
    input
        .state_mut()
        .with_symbol("name", name)
        .with_symbol("ns", &ns)
        .with_symbol("api", format!("https://{ns}/v2{}", request.uri().path()))
        .with_symbol("digest", digest);

    if let Some(accept) = request.header("accept") {
        input.state_mut().add_text_attr("accept", accept)
    }

    if let Some(response) = mirror_action.proxy(&mut input, request) {
        response
    } else {
        mirror_action
            .handle::<((Login, Authenticate), DownloadBlob)>(&mut input)
            .await
    }
}

#[derive(Deserialize)]
struct UploadParameters {
    digest: Option<String>,
    ns: String,
}
#[handler]
async fn blob_upload_chunks(
    request: &Request,
    method: Method,
    Path((name, reference)): Path<(String, String)>,
    Query(UploadParameters { digest, ns }): Query<UploadParameters>,
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
    input
        .state_mut()
        .with_symbol("name", name)
        .with_symbol("reference", reference)
        .with_symbol("api", format!("https://{ns}/v2{}", request.uri().path()))
        .with_symbol("digest", digest.unwrap_or_default());

    if let Some(response) = mirror_action.proxy(&mut input, request) {
        response
    } else {
        mirror_action
            .handle::<((Login, Authenticate), BlobUploadChunks)>(&mut input)
            .await
    }
}

#[derive(Deserialize)]
struct ImportParameters {
    digest: Option<String>,
    mount: Option<String>,
    from: Option<String>,
    ns: String,
}
#[handler]
async fn blob_upload(
    request: &Request,
    Path(name): Path<String>,
    Query(ImportParameters {
        digest,
        mount,
        from,
        ns,
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
        input
            .state_mut()
            .with_symbol("name", name)
            .with_symbol("mount", mount)
            .with_symbol("from", from)
            .with_symbol("api", format!("https://{ns}/v2{}", request.uri().path()));

        if let Some(response) = mirror_action.proxy(&mut input, request) {
            response
        } else {
            mirror_action
                .handle::<((Login, Authenticate), BlobImport)>(&mut input)
                .await
        }
    } else if let Some(digest) = digest {
        event!(
            Level::DEBUG,
            "Got blob_upload_monolith request, {name}, {digest}"
        );
        event!(Level::TRACE, "{:#?}", request);

        let mut input = dispatcher.clone();
        input
            .state_mut()
            .with_symbol("name", name)
            .with_symbol("digest", digest)
            .with_symbol("api", format!("https://{ns}/v2{}", request.uri().path()));

        if let Some(response) = mirror_action.proxy(&mut input, request) {
            response
        } else {
            mirror_action
                .handle::<((Login, Authenticate), BlobUploadMonolith)>(&mut input)
                .await
        }
    } else if let None = digest {
        event!(Level::DEBUG, "Got blob_upload_session_id request, {name}");
        event!(Level::TRACE, "{:#?}", request);

        let mut input = dispatcher.clone();
        input
            .state_mut()
            .with_symbol("name", name)
            .with_symbol("api", format!("https://{ns}/v2{}", request.uri().path()));

        if let Some(response) = mirror_action.proxy(&mut input, request) {
            response
        } else {
            mirror_action
                .handle::<((Login, Authenticate), BlobUploadSessionId)>(&mut input)
                .await
        }
    } else {
        soft_fail()
    }
}

/*
Table of OCI Endpoints

ID	Method	API Endpoint	Success	Failure
end-1	GET	/v2/	                                                                            200	404/401

end-2	GET / HEAD	/v2/<name>/blobs/<digest>	                                                200	404
end-10	DELETE	    /v2/<name>/blobs/<digest>	                                                202	404/405

end-4a	POST	    /v2/<name>/blobs/uploads/	                                                202	404
end-4b	POST	    /v2/<name>/blobs/uploads/             ?digest=<digest>	                    201/202	404/400
end-11	POST	    /v2/<name>/blobs/uploads/             ?mount=<digest>&from=<other_name>	    201	404

end-5	PATCH	    /v2/<name>/blobs/uploads/<reference>	                                    202	404/416
end-6	PUT	        /v2/<name>/blobs/uploads/<reference>  ?digest=<digest>	                    201	404/400

end-8a	GET	        /v2/<name>/tags/list	                                                    200	404
end-8b	GET	        /v2/<name>/tags/list                  ?n=<integer>&last=<integer>	        200	404

end-3	GET / HEAD	/v2/<name>/manifests/<reference>	                                        200	404
end-7	PUT	        /v2/<name>/manifests/<reference>	                                        201	404
end-9	DELETE	    /v2/<name>/manifests/<reference>	                                        202	404/400/405
*/

#[derive(Default)]
struct TestMirrorEvent;

impl MirrorProxy for TestMirrorEvent {
    fn resolve_response(_tc: &ThunkContext) -> Response {
        Response::builder().status(StatusCode::OK).finish()
    }

    fn resolve_error(_err: String, _tc: &ThunkContext) -> Response {
        Response::builder().status(StatusCode::OK).finish()
    }
}

#[test]
#[tracing_test::traced_test]
fn test_mirror() {
    use hyper::Client;
    use hyper_tls::HttpsConnector;
    use lifec::WorldExt;

    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let world = lifec::World::new();
        let entity = world.entities().create();
        let https = HttpsConnector::new();
        let client = Client::builder().build::<_, hyper::Body>(https);
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let handle = runtime.handle();
        let mut tc = ThunkContext::default()
            .enable_https_client(client)
            .enable_async(entity, handle.clone());

        let app = Mirror::<TestMirrorEvent>::create(&mut tc).routes();
        let cli = poem::test::TestClient::new(app);

        let resp = cli.get("/").send().await;
        resp.assert_status(StatusCode::NOT_FOUND);

        let resp = cli.head("/").send().await;
        resp.assert_status(StatusCode::NOT_FOUND);

        let resp = cli.get("/v2").send().await;
        resp.assert_status_is_ok();

        let resp = cli.get("/v2/").send().await;
        resp.assert_status_is_ok();

        let resp = cli.head("/v2").send().await;
        resp.assert_status_is_ok();

        let resp = cli.head("/v2/").send().await;
        resp.assert_status_is_ok();

        let resp = cli
            .get("/v2/library/test/manifests/test_ref?ns=test.com")
            .send()
            .await;
        resp.assert_status_is_ok();

        let resp = cli
            .head("/v2/library/test/manifests/test_ref?ns=test.com")
            .send()
            .await;
        resp.assert_status_is_ok();

        let resp = cli
            .put("/v2/library/test/manifests/test_ref?ns=test.com")
            .send()
            .await;
        resp.assert_status_is_ok();

        let resp = cli
            .delete("/v2/library/test/manifests/test_ref?ns=test.com")
            .send()
            .await;
        resp.assert_status_is_ok();

        // let resp = cli
        //     .get("/v2/library/test/blobs/test_digest?ns=test.com")
        //     .send()
        //     .await;
        // resp.assert_status_is_ok();

        // let resp = cli
        //     .post("/v2/library/test/blobs/uploads?ns=test.com")
        //     .send()
        //     .await;
        // resp.assert_status_is_ok();

        // let resp = cli
        //     .patch("/v2/library/test/blobs/uploads/test?ns=test.com")
        //     .send()
        //     .await;
        // resp.assert_status_is_ok();

        // let resp = cli
        //     .put("/v2/library/test/blobs/uploads/test?ns=test.com")
        //     .send()
        //     .await;
        // resp.assert_status_is_ok();

        // let resp = cli
        //     .get("/v2/library/test/tags/list?ns=test.com")
        //     .send()
        //     .await;
        // resp.assert_status_is_ok();

        runtime.shutdown_background();
    });
}
