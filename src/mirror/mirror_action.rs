use hyper::StatusCode;
use lifec::{ThunkContext, Plugin};
use poem::{Response, Request};

use crate::MirrorProxy;

/// Wrapper around mirror event functions
///
#[derive(Clone)]
pub struct MirrorAction {
    on_request: fn (&mut ThunkContext, &Request) -> Option<Response>,
    on_response: fn(tc: &ThunkContext) -> Response,
    on_error: fn(err: String, tc: &ThunkContext) -> Response,
}

impl MirrorAction {
    pub fn from<Event>() -> Self
    where
        Event: MirrorProxy + Default + Send + Sync + 'static,
    {
        MirrorAction {
            on_request: Event::on_request,
            on_response: Event::resolve_response,
            on_error: Event::resolve_error,
        }
    }

    ///  Receives request, if a response is returned then handle will not be called
    /// 
    pub fn proxy(&self, tc: &mut ThunkContext, request: &Request) -> Option<Response> {
        (self.on_request)(tc, request)
    }

    fn handle_response(&self, tc: &ThunkContext) -> Response {
        (self.on_response)(tc)
    }

    fn handle_error(&self, err: String, tc: &ThunkContext) -> Response {
        (self.on_error)(err, tc)
    }

    pub async fn handle<P>(&self, tc: &mut ThunkContext) -> Response
    where
        P: Plugin,
    {
        if let Some((task, _cancel)) = P::call(tc) {
            match task.await {
                Ok(result) => self.handle_response(&result),
                Err(err) => self.handle_error(format!("{}", err), &tc.clone()),
            }
        } else {
            soft_fail()
        }
    }
}

/// Fails in a way that the runtime will fallback to the upstream server
pub fn soft_fail() -> Response {
    Response::builder()
        .status(StatusCode::SERVICE_UNAVAILABLE)
        .finish()
}