use lifec::{ThunkContext, Thunk};
use poem::Response;

/// Implement to customize the response returned after calling the original api,
///
pub trait MirrorProxy {

    /// Called after the plugin finishes, and if the plugin returned the next thunk_context
    ///
    fn resolve_response(tc: &ThunkContext) -> Response;

    /// Called after the plugin finishes, and if the plugin task returned an error
    ///
    fn resolve_error(err: String, tc: &ThunkContext) -> Response;
}