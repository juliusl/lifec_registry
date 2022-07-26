
use lifec::{plugins::{ThunkContext, Plugin, Project}, Component, HashMapStorage, Runtime, editor::RuntimeEditor};
use lifec_poem::WebApp;
use poem::{Route, handler, web::{Path, Data, Query}, get, patch, post, http::{Method, self}, EndpointExt, Request, RequestBuilder, Response};
use serde::Deserialize;
use tracing::{instrument, event, Level};

use crate::{Resolve, ListTags, DownloadBlob, BlobUploadChunks, BlobUploadMonolith, BlobImport, BlobUploadSessionId, Upstream, create_runtime};


/// Designed to be used w/ containerd's registry config described here: 
/// https://github.com/containerd/containerd/blob/main/docs/hosts.md
/// 
/// 
#[derive(Component, Clone, Default)]
#[storage(HashMapStorage)]
pub struct Mirror(ThunkContext);

impl Plugin<ThunkContext> for Mirror
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
                if let Some(project) = tc.as_ref().find_text("project_src").and_then(|src| Project::load_file(src)) {
                   let block_name = tc.block.block_name.to_string();
                   if let Some(address) = tc.as_ref().find_text("address") {
                       let project =
                           project.with_block(&block_name, "app_host", |c| {
                               c.add_text_attr("address", &address);
                           });

                       let link = format!("https://{address}/v2");
                       let log = format!("Starting registry mirror on {link}");

                       tc.update_status_only(&log).await;
                       eprintln!("{log}");

                       let runtime = create_runtime(project);
                       let runtime_editor = RuntimeEditor::new(runtime);
                       // tc.as_mut().add_bool_attr("proxy_dispatcher", true);

                       let mut extension = Upstream::from(runtime_editor);
                       Runtime::start_with(&mut extension, Mirror::symbol(), &tc, cancel_source);
                   }
                }

                Some(tc)
            }
        })
    }
}

impl WebApp for Mirror
{
    fn create(context: &mut ThunkContext) -> Self {
        Self(context.clone())
    }

    fn routes(&mut self) -> Route {
        let context = &self.0;
        Route::new().nest("/v2", 
            Route::new()
                .at("/", get(index))
                .at("/:name/blobs/:digest", get(download_blob.data(context.clone())))
                .at("/:name/blobs/uploads", post(blob.data(context.clone())))
                .at("/:name/blobs/uploads/:reference", 
                    patch(blob_upload_chunks.data(context.clone()))
                    .put(blob_upload_chunks.data(context.clone()))
                    )
                .at("/:name/manifests/:reference", 
                    get(resolve.data(context.clone()))
                    .head(resolve.data(context.clone()))
                    .put(resolve.data(context.clone()))
                    .delete(resolve.data(context.clone())))
                .at("/:name/tags/list", get(list_tags.data(context.clone())))
        )
    }
}

#[handler]
async fn index() -> Response {
    event!(Level::TRACE, "Got /v2 request");
    Response::builder()
        .finish()
}

/// Resolves an image
#[handler]
async fn resolve(
    request: &Request,
    Path(name): Path<String>, 
    Path(reference): Path<String>, 
    dispatcher: Data<&ThunkContext>) -> Response
{
    // if let Some((task, _cancel)) = Resolve::call_with_context(&mut dispatcher.clone()) {
    //     let result = task.await;
    // }

    event!(Level::TRACE, "Got resolve request, {name} {reference}");
    Response::builder()
        .finish()
}

#[handler]
async fn list_tags(
    request: &Request,
    Path(name): Path<String>,
    dispatcher: Data<&ThunkContext>) -> Response 
{
    // if let Some((task, _cancel)) = ListTags::call_with_context(&mut dispatcher.clone()) {
    //     let result = task.await;
    // }

    event!(Level::TRACE, "Got list_tags request, {name}");
    Response::builder()
        .finish()
}

#[handler]
async fn download_blob(
    request: &Request,
    Path(name): Path<String>, 
    Path(digest): Path<String>, 
    dispatcher: Data<&ThunkContext>) -> Response 
{        
    // if let Some((task, _cancel)) = DownloadBlob::call_with_context(&mut dispatcher.clone()) {
    //     let result = task.await;
    // }
    event!(Level::TRACE, "Got resolve request, {name} {digest}");

    Response::builder()
        .finish()
}

#[derive(Deserialize)]
struct UploadParameters {
    digest: Option<String>
}

#[handler]
async fn blob_upload_chunks(
    request: &Request,
    method: Method,
    Path(name): Path<String>, 
    Path(reference): Path<String>, 
    Query(UploadParameters { digest }): Query<UploadParameters>, 
    dispatcher: Data<&ThunkContext>) -> Response 
{
    // if let Some((task, _cancel)) = BlobUploadChunks::call_with_context(&mut dispatcher.clone()) {
    //     let result = task.await;
    // }

    event!(Level::TRACE, "Got {method} blob_upload_chunks request, {name} {reference}");
    Response::builder()
        .finish()
}

#[derive(Deserialize)]
struct ImportParameters {
    digest: Option<String>,
    mount: Option<String>,
    from: Option<String>,
}
#[handler]
async fn blob(
    request: &Request,
    Path(name): Path<String>, 
    Query(ImportParameters { digest, mount, from }): Query<ImportParameters>, 
    dispatcher: Data<&ThunkContext>) -> Response 
{
    if let (Some(mount), Some(from)) = (mount, from) {
        event!(Level::TRACE, "Got blob_import request, {name}, {mount}, {from}");

        // let mut blob_import = dispatcher.clone();
        // blob_import.as_mut().with_text("repo", name).with_text("mount", mount).with_text("from", from);

        // if let Some((task, _cancel)) = BlobImport::call_with_context(&mut blob_import) {
        //     let result = task.await;
        // }
    } else if let Some(digest) = digest { 
        event!(Level::TRACE, "Got blob_upload_monolith request, {name}, {digest}");

        // if let Some((task, _cancel)) = BlobUploadMonolith::call_with_context(&mut dispatcher.clone()) {
        //     match task.await {
        //         Ok(_) => todo!(),
        //         Err(_) => todo!(),
        //     }
        // }
    } else if let None = digest { 
        event!(Level::TRACE, "Got blob_upload_session_id request, {name}");

        // if let Some((task, _cancel)) = BlobUploadSessionId::call_with_context(&mut dispatcher.clone()) {
        //     match task.await {
        //         Ok(_) => todo!(),
        //         Err(_) => todo!(),
        //     }
        // }
    }

    Response::builder()
        .finish()
}

// Endpoints

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