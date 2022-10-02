use std::fmt::Display;

use lifec::{AttributeParser, WorldExt, Value};
use logos::Logos;

use super::resources::Resources;

/// Enumeration of methods to proxy
/// 
#[derive(Logos)]
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
    pub fn parse_methods(resource: Resources) -> impl Fn(&mut AttributeParser, String) {
        move |parser: &mut AttributeParser, content: String| {
            let mut lexer = Methods::lexer(content.as_ref());

            // This will indicate where to look to find each route
            let route_entity = parser.world()
                .expect("should exist")
                .entities()
                .create();
            parser.define("route", route_entity.id() as usize);
            
            // Each route will get it's own engine
            // By setting the id here, plugins will add to the route_entity's properties
            parser.set_id(route_entity.id());

            while let Some(token) = lexer.next() {
                parser.define("method", Value::Symbol(format!("{token}")));

                match (&token, &resource) {
                    (Methods::Head | Methods::Delete | Methods::Get | Methods::Put, Resources::Manifests) => {
                        parser.define("path", "/:name<[a-zA-Z0-9/_-]+(?:manifests)>/:reference");
                        parser.define("resource", "manifests");
                    },
                    (Methods::Get, Resources::Blobs) => {
                        parser.define("path", "/:name<[a-zA-Z0-9/_-]+(?:manifests)>/:reference");
                        parser.define("resource", "blobs");
                    }
                    (Methods::Put | Methods::Patch, Resources::Blobs) => {
                        parser.define("path", "/:name<[a-zA-Z0-9/_-]+(?:blobs)>/uploads/:reference");
                        parser.define("resource", "blobs");
                    }
                    (Methods::Post, Resources::Blobs) => {
                        parser.define("path", "/:name<[a-zA-Z0-9/_-]+(?:blobs)>/uploads");
                        parser.define("resource", "blobs");
                    }
                    (Methods::Get, Resources::Tags) => {
                        parser.define("path", "/:name<[a-zA-Z0-9/_-]+(?:tags)>/list");                        
                        parser.define("resource", "tags");
                    }
                    _ => continue
                }
            }
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

