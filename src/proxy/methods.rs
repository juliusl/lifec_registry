use std::{fmt::Display, path::PathBuf};

use lifec::prelude::{AttributeParser, BlockIndex, Value, WorldExt, CustomAttribute};
use logos::Logos;
use tracing::{event, Level};

use super::resources::Resources;

/// Enumeration of methods to proxy,
/// 
/// This is a lexer to read input from .host attributes,
///
#[derive(Logos, Debug, Clone, Hash, PartialEq, Eq)]
pub enum Methods {
    #[token("head")]
    Head,
    #[token("get")]
    Get,
    #[token("post")]
    Post,
    #[token("put")]
    Put,
    #[token("patch")]
    Patch,
    #[token("delete")]
    Delete,
    #[error]
    #[regex(r"[ \t\n\f]+", logos::skip)]
    Error,
}

impl Methods {
    /// Returns a function for parsing a proxy route handler definition for a specified resource (manifests, blobs, tags, etc),
    ///  
    pub fn parse_methods(resource: Resources) -> impl Fn(&mut AttributeParser, String) {
        move |parser: &mut AttributeParser, content: String| {
            let mut lexer = Methods::lexer(content.as_ref());

            // This will indicate where to look to find each route
            let route_entity = parser.world().expect("should exist").entities().create();

            let clone = parser.clone();
            let index = BlockIndex::index(clone);
            let index = index.first().expect("should be an index at this point");
            let current_root = parser.entity().expect("should be set at this point");

            // Each route will get it's own engine
            // By setting the id here, plugins will add to the route_entity's properties
            if let Some(proxy_entity) = index
                .properties()
                .property("proxy_entity")
                .and_then(|p| p.int())
            {
                parser.set_id(proxy_entity as u32);
                parser.define("route", route_entity.id() as usize);
                parser.set_id(route_entity.id());
                parser.define("proxy_entity", proxy_entity as usize);
            } else {
                parser.define("route", route_entity.id() as usize);
                parser.set_id(route_entity.id());
                parser.define("proxy_entity", current_root.id() as usize);
            }

            while let Some(token) = lexer.next() {
                match token {
                    Methods::Error => continue,
                    _ => {}
                }

                match (&token, &resource) {
                    (
                        Methods::Head | Methods::Delete | Methods::Get | Methods::Put,
                        Resources::Manifests,
                    ) => {
                        parser.define("method", Value::Symbol(format!("{token}")));
                        parser.define("resource", "manifests");
                    }
                    (Methods::Get, Resources::Blobs) => {
                        parser.define("method", Value::Symbol(format!("{token}")));
                        parser.define("resource", "blobs");
                    }
                    (Methods::Put | Methods::Patch, Resources::Blobs) => {
                        parser.define("method", Value::Symbol(format!("{token}")));
                        parser.define("resource", "blobs");
                    }
                    (Methods::Post, Resources::Blobs) => {
                        parser.define("method", Value::Symbol(format!("{token}")));
                        parser.define("resource", "blobs");
                    }
                    (Methods::Get, Resources::Tags) => {
                        parser.define("method", Value::Symbol(format!("{token}")));
                        parser.define("resource", "tags");
                    }
                    _ => continue,
                }
            }

            // This allows hot reoloading of proxy src so that the host the proxy uses can be
            // edited without restarting the server. However, this will only work for
            // modifying values, and will not allow the actual plugin sequence to be modified
            parser.add_custom(CustomAttribute::new_with("proxy_src_path", |p, content| {
                let path = PathBuf::from(content);

                // The path must actually exist or this will not be set
                if let Some(path) = path.canonicalize().ok() {
                    event!(
                        Level::DEBUG,
                        "Adding proxy_src path for entity -> {:?}",
                        p.entity(),
                    );
                    p.define(
                        "proxy_src_path",
                        Value::Symbol(path.to_str().expect("should be a string").to_string()),
                    );
                } else {
                    event!(
                        Level::ERROR,
                        "proxy_src_path was not set because the given path does not exist"
                    );
                }
            }));
        }
    }
}

impl Display for Methods {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Methods::Head => write!(f, "head"),
            Methods::Get => write!(f, "get"),
            Methods::Post => write!(f, "post"),
            Methods::Put => write!(f, "put"),
            Methods::Patch => write!(f, "patch"),
            Methods::Delete => write!(f, "delete"),
            Methods::Error => unreachable!(),
        }
    }
}
