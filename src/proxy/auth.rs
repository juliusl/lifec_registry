
pub struct Auth {
    remote_url: String
}

#[handler]
async fn auth_api(
    request: &poem::Request,
    Query(Auth { remote_url }): Query<Auth>,
    host: Data<&Arc<Host>>,
    context: Data<&ThunkContext>,
) -> Response {

    /*
    a) An access token
    b) Call authenticate
    */

    let mut registry = host.world().system_data::<Registry>();

    registry
        .proxy_request::<Manifests>(
            &context,
            resolve
                .operation
                .clone()
                .expect("should have an operation name"),
            request,
            Some(body.into()),
            ns,
            repo.trim_end_matches("/manifests"),
            reference,
        ).await
}
