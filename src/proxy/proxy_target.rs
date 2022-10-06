use std::{fmt::Display, str::FromStr};

use hyper::{Body, Method, Response};
use lifec::{AttributeIndex, MemoryBlobSource, ThunkContext};
use logos::{Lexer, Logos};
use poem::{web::headers::Authorization, Request, RequestBuilder};
use tracing::{event, Level};

use crate::{
    content::{Descriptor, Manifests},
    ArtifactManifest, ImageIndex, ImageManifest,
};

/// Wrapper struct w/ important properties from the proxied request,
///
/// This represents the upstream server.
///
#[derive(Debug)]
pub struct ProxyTarget {
    /// From the request query `ns` parameter,
    ///
    pub namespace: String,
    /// Repository name,
    ///
    pub repo: String,
    /// Proxied api
    ///
    pub api: String,
    /// Proxied method
    ///
    pub method: String,
    /// Parent thunk context this struct was created from,
    ///
    pub thunk_context: ThunkContext,
    /// This is the object portion of the proxied request, typically a reference (tag) or digest
    ///
    object: Object,
    /// This is the media settings of the proxied request,
    ///
    media: Media,
}

impl ProxyTarget {
    /// Continues the request, if successful returns self, otherwise returns None
    ///
    pub async fn continue_request(&self) -> Option<Response<Body>> {
        if let Some(request) = self.start_request() {
            match &self.media {
                Media::Accept(accept) => {
                    let request = request
                        .header("accept", accept)
                        .uri_str(self.api.as_str())
                        .finish();

                    self.send_request(request).await
                }
                Media::ContentType { content_type, body } => {
                    let request = request
                        .content_type(content_type)
                        .method(
                            Method::from_str(self.method.as_str()).expect("should be valid method"),
                        )
                        .body(body.to_vec());

                    self.send_request(request).await
                }
                Media::None => {
                    event!(Level::WARN, "No media to continue request w/");
                    None
                }
            }
        } else {
            event!(
                Level::ERROR,
                "Could not continue request w/o authentication"
            );
            None
        }
    }

    /// Transform target into a descriptor,
    ///
    pub async fn resolve(&self) -> Option<Manifests> {
        match &self.media {
            Media::Accept(accept)
                if matches!(self.thunk_context.search().find_symbol("digest"), Some(_)) =>
            {
                let digest = self
                    .thunk_context
                    .search()
                    .find_symbol("digest")
                    .expect("should have a digest");
                let request = self
                    .start_request()
                    .expect("should be able to start a request")
                    .header("accept", accept)
                    .method(Method::GET)
                    .uri_str(self.manifest_with(digest).as_str())
                    .finish();

                self.resolve_manifest(request).await
            }
            Media::Accept(accept) => {
                // Don't have a digest yet, so we'll need to resolve this
                // Also, this means this is a manifest

                let request = self
                    .start_request()
                    .expect("should be able to start a request")
                    .header("accept", accept)
                    .method(Method::GET)
                    .uri_str(self.manifest_url().as_str())
                    .finish();

                self.resolve_manifest(request).await
            }
            Media::ContentType { content_type, body } => todo!(),
            Media::None => None,
        }
    }

    pub async fn resolve_manifest(&self, request: Request) -> Option<Manifests> {
        match self.send_request(request).await {
            Some(resp) if resp.status().is_success() => {
                let digest = resp
                    .headers()
                    .get("Docker-Content-Digest")
                    .expect("should have a digest header")
                    .to_str()
                    .expect("should be a string")
                    .to_string();

                let body = Self::parse_body(resp).await.expect("should exist");

                if let Some(image_index) =
                    serde_json::from_slice::<ImageIndex>(body.as_slice()).ok()
                {
                    let manifest = image_index.clone();
                    Some(Manifests::Index(
                        Descriptor {
                            media_type: image_index.media_type,
                            artifact_type: None,
                            digest: digest.to_string(),
                            size: body.len() as u64,
                            annotations: None,
                            urls: None,
                            data: None,
                            platform: None,
                        },
                        manifest,
                    ))
                } else if let Some(image_manifest) =
                    serde_json::from_slice::<ImageManifest>(body.as_slice()).ok()
                {
                    let manifest = image_manifest.clone();
                    Some(Manifests::Image(
                        Descriptor {
                            media_type: image_manifest.media_type,
                            artifact_type: None,
                            digest: digest.to_string(),
                            size: body.len() as u64,
                            annotations: image_manifest.annotations,
                            urls: None,
                            data: None,
                            platform: None,
                        },
                        manifest,
                    ))
                } else if let Some(artifact_manifest) =
                    serde_json::from_slice::<ArtifactManifest>(body.as_slice()).ok()
                {
                    let manifest = artifact_manifest.clone();
                    Some(Manifests::Artifact(
                        Descriptor {
                            media_type: artifact_manifest.media_type,
                            artifact_type: Some(artifact_manifest.artifact_type),
                            digest: digest.to_string(),
                            size: body.len() as u64,
                            annotations: artifact_manifest.annotations,
                            urls: None,
                            data: None,
                            platform: None,
                        },
                        manifest,
                    ))
                } else {
                    None
                }
            }
            Some(resp) if resp.status().is_server_error() => {
                // Is there an outage? If so can we retry later?
                None
            }
            Some(resp) if resp.status().is_client_error() => {
                // Shouldn't retry on a client error
                None
            }
            _ => None,
        }
    }

    /// Request content w/ a descriptor from the proxy target,
    ///
    pub async fn request_content(&self, descriptor: &Descriptor) -> Option<Vec<u8>> {
        let client = self
            .thunk_context
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
            .expect("should return a request builder")
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

    pub async fn parse_body(response: Response<Body>) -> Option<Vec<u8>> {
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

    /// Starts an authenticated requets to the proxy target,
    ///
    pub fn start_request(&self) -> Option<RequestBuilder> {
        match Authorization::bearer(
            self.thunk_context
                .search()
                .find_symbol("access_token")
                .expect("should have an access token")
                .as_str(),
        ) {
            Ok(auth_header) => Some(Request::builder().typed_header(auth_header)),
            Err(err) => {
                event!(Level::ERROR, "Could not parse auth header, {err}");
                None
            }
        }
    }

    /// Sends a request (https only),
    ///
    pub async fn send_request(&self, request: Request) -> Option<Response<Body>> {
        if let Some(client) = self.thunk_context.client() {
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

    /// Returns a blob upload url,
    ///
    pub fn blob_upload_url(&self) -> String {
        let Self {
            namespace, repo, ..
        } = self;

        format!("https://{namespace}/v2/{repo}/blobs/upload")
    }

    pub fn manifest_url(&self) -> String {
        let Self {
            namespace,
            repo,
            object,
            ..
        } = self;

        let repo = if let Some(import) = self.thunk_context.search().find_symbol("import") {
            import
        } else {
            repo.to_string()
        };

        format!("https://{namespace}/v2/{repo}/manifests/{object}")
    }

    pub fn manifest_with(&self, object: impl AsRef<str>) -> String {
        let Self {
            namespace,
            repo,
            ..
        } = self;

        format!(
            "https://{namespace}/v2/{repo}/manifests/{}",
            object.as_ref()
        )
    }
}

#[derive(Logos)]
enum Resource {
    /// Manifests
    ///
    #[token("application/vnd.oci.image.manifest.v1+json")]
    #[token("application/vnd.oci.artifact.manifest.v1+json")]
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

#[derive(Logos, Debug, PartialEq, Eq)]
enum Object {
    /// From OCI documentation,
    ///
    /// ```quote
    /// Throughout this document, <reference> as a tag MUST be at most 128 characters in length and MUST match the following regular expression:
    ///[a-zA-Z0-9_][a-zA-Z0-9._-]{0,127}
    /// ```
    ///
    #[regex("[a-zA-Z0-9_][a-zA-Z0-9._-]+", on_reference)]
    Reference(String),
    /// Parses a sha-digest, currently 256 and 512 are supported
    ///
    #[regex("sha512:[a-f0-9]+", on_digest)]
    #[regex("sha256:[a-f0-9]+", on_digest)]
    Digest(String),
    #[error]
    #[regex(r"[ \t\n\f]+", logos::skip)]
    Error,
}

impl Display for Object {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Object::Reference(reference) => write!(f, "{reference}"),
            Object::Digest(digest) => write!(f, "{digest}"),
            Object::Error => panic!("Is nto a valid object for display"),
        }
    }
}

/// Enumeration of types of media this proxy target is dealing with
///
#[derive(Debug)]
enum Media {
    /// If the proxied request is trying to get content,
    ///
    /// Then these are the expected media types. Can be a comma delimitted list of media types,
    ///
    Accept(String),
    /// If the proxied request is trying to upload content,
    ///
    /// Then this tuple is the content-type and body
    ///
    ContentType {
        content_type: String,
        body: Vec<u8>,
    },
    None,
}

fn on_reference(lexer: &mut Lexer<Object>) -> Option<String> {
    if lexer.slice().len() > 128 {
        None
    } else {
        Some(lexer.slice().to_string())
    }
}

fn on_digest(lexer: &mut Lexer<Object>) -> Option<String> {
    let digest = &lexer.remainder()[..];

    if lexer.slice().contains("sha256") {
        assert!(digest.len() < 64);
    } else if lexer.slice().contains("sha512") {
        assert!(digest.len() < 128);
    } else {
        panic!("unspported")
    }

    Some(format!("{}{}", lexer.slice(), digest))
}

impl TryFrom<&ThunkContext> for ProxyTarget {
    type Error = ();

    fn try_from(tc: &ThunkContext) -> Result<Self, Self::Error> {
        if let (Some(namespace), Some(repo), Some(api)) = (
            tc.search().find_symbol("ns"),
            tc.search().find_symbol("name"),
            tc.search().find_symbol("api"),
        ) {
            Ok(Self {
                namespace,
                repo,
                api,
                object: {
                    if let Some(digest) = tc.search().find_symbol("digest") {
                        Object::Digest(digest)
                    } else if let Some(reference) = tc.search().find_symbol("reference") {
                        Object::lexer(&reference).next().unwrap_or(Object::Error)
                    } else {
                        Object::Error
                    }
                },
                media: {
                    if let Some(accept) = tc.search().find_symbol("accept") {
                        Media::Accept({
                            if let Some(resolve) = tc.state().find_symbol("resolve") {
                                event!(Level::DEBUG, "Setting accept to {resolve}");
                                resolve
                            } else {
                                accept
                            }
                        })
                    } else if let Some(body) = tc.search().find_binary("body") {
                        let content_type = tc
                            .search()
                            .find_symbol("content-type")
                            .expect("should have a content_type if there's a body");
                        Media::ContentType { content_type, body }
                    } else {
                        Media::None
                    }
                },
                method: {
                    if let Some(method) = tc.search().find_symbol("method") {
                        method.to_uppercase()
                    } else {
                        String::from("GET")
                    }
                },
                thunk_context: tc.clone(),
            })
        } else {
            Err(())
        }
    }
}

#[test]
fn test_object_parser() {
    // Test digests
    let mut lexer =
        Object::lexer("sha256:b94d27b9934d3e8a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9");

    assert_eq!(
        lexer.next(),
        Some(Object::Digest(
            "sha256:b94d27b9934d3e8a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9".to_string()
        ))
    );

    let mut lexer =
        Object::lexer("sha256:c93e919e9985d48c6142530fa902745b76b28873488a64f9422302c620d170");

    assert_eq!(
        lexer.next(),
        Some(Object::Digest(
            "sha256:c93e919e9985d48c6142530fa902745b76b28873488a64f9422302c620d170".to_string()
        ))
    );

    // Test tags
    let mut lexer = Object::lexer("demo_.thats-really_cool");

    assert_eq!(
        lexer.next(),
        Some(Object::Reference("demo_.thats-really_cool".to_string()))
    );

    // Test tags with numbers
    let mut lexer = Object::lexer("9demo_.thats-reall8y_cool");

    assert_eq!(
        lexer.next(),
        Some(Object::Reference("9demo_.thats-reall8y_cool".to_string()))
    );

    // Test tags with starting underscore
    let mut lexer = Object::lexer("_9demo_.thats-reall8y_cool");

    assert_eq!(
        lexer.next(),
        Some(Object::Reference("_9demo_.thats-reall8y_cool".to_string()))
    );
}