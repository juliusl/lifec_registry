use crate::Proxy;
use hyper::http::StatusCode;
use lifec::{
    AttributeIndex, 
    Host, 
    ThunkContext,
    Executor
};
use poem::{
    handler,
    web::{Data, Path, Query},
    Request, Response,
};
use serde::Deserialize;
use tracing::{event, Level};

#[derive(Deserialize)]
pub struct TagsAPIParams {
    ns: String,
}
#[handler]
pub async fn tags_api(
    request: &Request,
    Path(name): Path<String>,
    Query(TagsAPIParams { ns }): Query<TagsAPIParams>,
    context: Data<&ThunkContext>,
) -> Response {
    if !context.is_enabled("proxy_enabled") {
        return Response::builder()
            .status(StatusCode::SERVICE_UNAVAILABLE)
            .finish();
    }

    let name = name.trim_end_matches("/tags");

    event!(Level::DEBUG, "Got list_tags request, {name}");
    event!(Level::TRACE, "{:#?}", request);

    let mut input = context.clone();
    input
        .state_mut()
        .with_symbol("ns", ns)
        .with_symbol("name", name);

    let mut host = Host::load_content::<Proxy>(
        input.state().find_text("proxy_src").unwrap()
    );

    let input = host.execute(&input);
    Proxy::into_response(&input)
}