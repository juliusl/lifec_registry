use std::{fmt::Display, path::PathBuf, str::FromStr};

use hyper::{Method, Response};
use lifec::prelude::{AttributeIndex, ThunkContext};
use logos::Logos;
use poem::{Body, Request, RequestBuilder};
use tracing::{event, Level};

use crate::content::Descriptor;

mod object;
pub use object::Object;

/// Wrapper struct representing properties of the upstream server,
///
#[derive(Debug)]
pub struct ProxyTarget {
    /// From the request query `ns` parameter,
    ///
    namespace: String,
    /// Repository name,
    ///
    repo: String,
    /// Parent thunk context this struct was created from,
    ///
    context: ThunkContext,
    /// This is the object portion of the proxied request, typically a reference (tag) or digest
    ///
    object: Object,
}

impl ProxyTarget {
    /// Returns the current object setting,
    /// 
    pub fn object(&self) -> &Object {
        &self.object
    }

    /// Request content w/ a descriptor from the proxy target,
    ///
    pub async fn request_content(&self, descriptor: &Descriptor) -> Option<Vec<u8>> {
        let client = self
            .context
            .client()
            .expect("should have a client to make requests");

        let Self {
            namespace, repo, ..
        } = self;

        let Descriptor {
            media_type, digest, ..
        } = descriptor;

        let resource = Resource::lexer(media_type)
            .next()
            .expect("should return something");

        let resource_url = format!("https://{namespace}/v2/{repo}/{resource}/{digest}");

        let req = self
            .start_request()
            .uri_str(resource_url)
            .header("accept", media_type)
            .finish();

        if let Some(mut response) = self.send_request(req).await {
            if let Some(location) = response.headers().get("Location") {
                event!(Level::DEBUG, "Following redirect from location header");
                response = client
                    .get(location.to_str().unwrap_or_default().parse().unwrap())
                    .await
                    .unwrap()
            };

            Self::parse_body(response).await
        } else {
            None
        }
    }

    /// Resolves a descriptor from a uri,
    ///
    pub async fn resolve_descriptor(&self, uri: impl AsRef<str>) -> Option<Descriptor> {
        let url = hyper::Uri::from_str(uri.as_ref()).expect("should be a valid uri");

        // Check the uri we're passed has the same host as the upstream server we're targeting
        if url.host().unwrap_or_default() != self.namespace {
            panic!("Uri passed is a different host then the current proxy target");
        }

        let accept = self
            .context
            .search()
            .find_symbol("accept")
            .expect("should have accept");
        
        let request = self
            .start_request()
            .uri_str(uri.as_ref())
            .header("accept", &accept)
            .method(Method::HEAD)
            .finish();

        self.send_request(request).await.and_then(|resp| {
            if resp.status().is_success() {
                let digest = resp
                    .headers()
                    .get("docker-content-digest")
                    .expect("should have a digest")
                    .to_str()
                    .expect("should be a string");

                let content_lengtth = resp
                    .headers()
                    .get("content-length")
                    .expect("should have a content length")
                    .to_str()
                    .expect("should be a string")
                    .parse::<u64>()
                    .expect("should be an integer");

                let content_type = resp
                    .headers()
                    .get("content-type")
                    .expect("should have a content type")
                    .to_str()
                    .expect("should be a string");

                let desc = Descriptor {
                    media_type: content_type.to_string(),
                    artifact_type: None,
                    digest: digest.to_string(),
                    size: content_lengtth,
                    annotations: None,
                    urls: None,
                    data: None,
                    platform: None,
                };

                Some(desc)
            } else {
                None
            }
        })
    }

    /// Starts an authenticated requets to the proxy target,
    ///
    pub fn start_request(&self) -> RequestBuilder {
        let auth = self
            .context
            .search()
            .find_symbol("Authorization")
            .expect("should have authorization");
            
       Request::builder().header("authorization", &auth)
    }

    /// Sends a request (https only),
    ///
    pub async fn send_request(&self, request: Request) -> Option<Response<hyper::Body>> {
        if let Some(client) = self.context.client() {
            event!(Level::TRACE, "Sending request, {:#?}", &request);
            match client.request(request.into()).await {
                Ok(response) => {
                    event!(Level::TRACE, "Received response, {:#?}", response);
                    Some(response)
                }
                Err(err) => {
                    event!(Level::ERROR, "Error making request {err}");
                    None
                }
            }
        } else {
            None
        }
    }

    /// Returns a blob upload url to the upstream target,
    ///
    pub fn blob_upload_url(&self) -> String {
        let Self {
            namespace, repo, ..
        } = self;

        format!("https://{namespace}/v2/{repo}/blobs/upload")
    }

    /// Returns a blob url to the upstream target,
    ///
    pub fn blob_url(&self) -> String {
        let Self {
            namespace,
            repo,
            object,
            ..
        } = self;

        format!("https://{namespace}/v2/{repo}/blobs/{object}")
    }

    /// Returns a referrers url, does not filter artifact_type
    /// 
    pub fn referrers_url(&self) -> String {
        let Self {
            namespace,
            repo,
            object,
            ..
        } = self;

        format!("https://{namespace}/v2/{repo}/_oras/artifacts/referrers?digest={object}")
    }

    /// Returns a manifest url to the upstream target,
    ///
    pub fn manifest_url(&self) -> String {
        let Self {
            namespace,
            repo,
            object,
            ..
        } = self;

        let repo = if let Some(import) = self.context.search().find_symbol("import") {
            import
        } else {
            repo.to_string()
        };

        format!("https://{namespace}/v2/{repo}/manifests/{object}")
    }

    /// Returns a manifest url with a specific object to the upstream target,
    ///
    pub fn manifest_with(&self, object: impl AsRef<str>) -> String {
        let Self {
            namespace, repo, ..
        } = self;

        format!(
            "https://{namespace}/v2/{repo}/manifests/{}",
            object.as_ref()
        )
    }

    /// Returns an image reference for this target,
    ///
    pub fn image_reference(&self) -> String {
        let Self {
            namespace,
            repo,
            object,
            ..
        } = self;

        format!("{namespace}/{repo}{:#}", object)
    }

    /// Returns an image reference for this target w/ a different object,
    ///
    pub fn image_reference_with(&self, object: impl Into<Object>) -> String {
        let ob = object.into();
        let Self {
            namespace, repo, ..
        } = self;

        format!("{namespace}/{repo}{:#}", ob)
    }

    /// Reads the body from a response and returns the bytes,
    ///
    async fn parse_body(response: Response<hyper::Body>) -> Option<Vec<u8>> {
        match hyper::body::to_bytes(response.into_body()).await {
            Ok(data) => {
                event!(Level::DEBUG, "Resolved blob, len: {}", data.len());
                Some(data.to_vec())
            }
            Err(err) => {
                event!(Level::ERROR, "{err}");
                None
            }
        }
    }
}

/// Enumeration of well known resource types,
/// 
#[derive(Logos)]
enum Resource {
    /// Known supported manifest types,
    ///
    #[token("application/vnd.oci.artifact.manifest.v1+json")]
    #[token("application/vnd.cncf.oras.artifact.manifest.v1+json")]
    #[token("application/vnd.oci.image.manifest.v1+json")]
    #[token("application/vnd.docker.distribution.manifest.v1+json")]
    #[token("application/vnd.docker.distribution.manifest.v2+json")]
    #[token("application/vnd.docker.distribution.manifest.list.v2+json")]
    Manifest,
    /// Non-exhaustive list of blobs, will default to this
    ///
    #[token("application/vnd.oci.image.layer.v1.tar")]
    #[token("application/vnd.docker.container.image.v1+json")]
    #[token("application/vnd.docker.image.rootfs.diff.tar.gzip")]
    // #[token("application/vnd.docker.image.rootfs.foreign.diff.tar.gzip")]
    #[token("application/vnd.docker.plugin.v1+json")]
    #[token("application/vnd.oci.image.config.v1+json")]
    #[token("application/vnd.oci.image.layer.v1.tar+gzip")]
    #[token("application/vnd.oci.image.layer.v1.tar+zstd")]
    #[token("application/gzip")]
    #[token("application/octet-stream")]
    #[token("application/json")]
    #[token("text/plain")]
    Blobs,
    #[error]
    #[regex(r"[ \t\n\f]+", logos::skip)]
    Error,
}

impl Display for Resource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Resource::Manifest => write!(f, "manifests"),
            Resource::Blobs => write!(f, "blobs"),
            Resource::Error => write!(f, "blobs"),
        }
    }
}


impl From<&Request> for ProxyTarget {
    fn from(req: &Request) -> Self {
        let ns = req.uri().host().expect("should have a host");
        let path = req
            .uri()
            .path()
            .parse::<PathBuf>()
            .ok()
            .expect("should parse to a path buf");

        let reference = path
            .file_name()
            .expect("should have a reference")
            .to_str()
            .expect("should be a string")
            .to_string();
        let repo = path
            .parent()
            .expect("should have a repo component")
            .to_str()
            .expect("should be a string")
            .to_string();

        Self {
            namespace: ns.to_string(),
            repo,
            context: ThunkContext::default(),
            object: {
                match Object::lexer(&reference).next() {
                    Some(obj) => obj,
                    None => panic!("A reference is required"),
                }
            },
        }
    }
}

impl TryFrom<&ThunkContext> for ProxyTarget {
    type Error = crate::Error;

    fn try_from(tc: &ThunkContext) -> Result<Self, Self::Error> {
        if let (Some(namespace), Some(repo)) = (
            tc.search().find_symbol("REGISTRY_NAMESPACE"),
            tc.search().find_symbol("REGISTRY_REPO"),
        ) {
            Ok(Self {
                namespace,
                repo,
                object: {
                    if let Some(digest) = tc.search().find_symbol("digest") {
                        Object::Digest(digest)
                    } else if let Some(reference) = tc.search().find_symbol("REFERENCE") {
                        Object::lexer(&reference).next().unwrap_or(Object::Error)
                    } else {
                        Object::Error
                    }
                },
                context: tc.clone(),
            })
        } else {
            Err(crate::Error::invalid_operation("Current context is missing namespace, repo information"))
        }
    }
}

impl From<(&Request, Body)> for ProxyTarget {
    fn from((_, _): (&Request, Body)) -> Self {
        todo!()
    }
}
