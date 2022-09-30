use lifec::{ThunkContext};
use poem::{Response, Request};

/// Implement to customize the response returned after calling the original api,
///
pub trait MirrorProxy {
    /// This will be called when a request is received, after propeties are added to state,
    /// but before the runtime has a chance to make the request to the upstream server. 
    /// 
    /// If a response is returned from this function, then the runtime will skip over
    /// any actions and return the response. 
    /// 
    fn on_request(_tc: &ThunkContext, _request: &Request) -> Option<Response> {
        None
    }

    /// Called after the plugin finishes, and if the plugin returned the next thunk_context
    ///
    fn resolve_response(tc: &ThunkContext) -> Response;

    /// Called after the plugin finishes, and if the plugin task returned an error
    ///
    fn resolve_error(err: String, tc: &ThunkContext) -> Response;
}