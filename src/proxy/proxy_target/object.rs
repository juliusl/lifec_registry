use std::fmt::Display;

use logos::{Lexer, Logos};

/// Enumeration of object types for repos, can either be a reference tag or sha digest
/// 
#[derive(Logos, Debug, PartialEq, Eq)]
pub enum Object {
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
            Object::Reference(reference) => {
                if f.alternate() {
                    write!(f, ":{reference}")
                } else {
                    write!(f, "{reference}")
                }
            }
            Object::Digest(digest) => {
                if f.alternate() {
                    write!(f, "@{digest}")
                } else {
                    write!(f, "{digest}")
                }
            }
            Object::Error => panic!("Is nto a valid object for display"),
        }
    }
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

