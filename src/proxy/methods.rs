use lifec::{AttributeParser, WorldExt};
use logos::Logos;

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
    #[token("delete")]
    Delete,
    #[error]
    #[regex(r"[ \t\n\f]+", logos::skip)]
    Error,
}

impl Methods {
    pub fn parse_methods(type_name: &'static str) -> impl Fn(&mut AttributeParser, String) {
        move |parser: &mut AttributeParser, content: String| {
            let mut lexer = Methods::lexer(content.as_ref());

            let route = parser.world().expect("should exist").entities().create();
    
            parser.define_child(route, "route", type_name);
    
            while let Some(token) = lexer.next() {
                parser.define_child(route, "method", match token {
                    Methods::Head => "head",
                    Methods::Get => "get",
                    Methods::Post => "post",
                    Methods::Put => "put",
                    Methods::Delete => "delete",
                    Methods::Error => "",
                });
            }
        }
   
    }
}
