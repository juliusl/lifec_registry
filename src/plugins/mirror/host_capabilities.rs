use logos::Logos;


/// Host capabilities for configuring hosts.toml
/// 
#[derive(Logos)]
pub enum HostCapability {
    #[token("resolve")]
    Resolve,
    #[token("push")]
    Push,
    #[token("pull")]
    Pull,
    #[error]
    #[regex(r"[ \t\n\f]+", logos::skip)]
    Error,
}