use logos::Logos;

/// Enumeration of api resources to proxy
/// 
#[derive(Clone, Logos)]
pub enum Resources {
    #[token("manifests")]
    Manifests,
    #[token("blobs")]
    Blobs,
    #[token("tags")]
    Tags,
    #[error]
    Error,
}
