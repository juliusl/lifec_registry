use lifec::{ThunkContext, AttributeIndex, Project, Host};
use lifec_poem::WebApp;
use poem::Route;

/// Struct for creating a customizable registry proxy
/// 
pub struct Proxy {
    context: ThunkContext
}

impl Project for Proxy {
    fn configure_engine(_: &mut lifec::Engine) {
        // 
    }

    fn interpret(_: &lifec::World, _: &lifec::Block) {
        // 
    }

    fn configure_dispatcher(dispatcher_builder: &mut lifec::DispatcherBuilder, context: Option<ThunkContext>) {
        if let Some(tc) = context {
            Host::add_error_context_listener::<Proxy>(
                tc, 
                dispatcher_builder
            );
        }
    }
}
impl WebApp for Proxy {
    fn create(context: &mut lifec::ThunkContext) -> Self {
        Self {
            context: context.clone()
        }
    }

    fn routes(&mut self) -> poem::Route {
        self.context.state().find_bool("todo");
        /* 
        Initial design
        ``` start proxy
        + .runtime
        : .println  Starting proxy
        : .mirror   <>
        : .host     <>
        : .host     <>

        + .proxy                  teleport    
        : .route                  <prefix>          
        : .method                 head, get         
        : .login                  access_token      
        : .authenticate           oauth2            
        - or, : .login            rsa.key     
        - or, : .authenticate     cert        
        : .resolve          
        */

        Route::new()
    }
}

impl From<ThunkContext> for Proxy { 
    fn from(context: ThunkContext) -> Self {
        Self {
            context
        }
    }
}
