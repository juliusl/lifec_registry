// Imports
use hyper::Method;
use lifec::state::AttributeIndex;
use lifec::prelude::ThunkContext;
use poem::IntoResponse;
use poem::web::Query;
use poem::web::Data;
use poem::handler;
use poem::error::IntoResult;
use serde::Serialize;
use serde::Deserialize;
use tracing::info;
use tracing::error;
use tracing::debug;
use crate::Error;
use crate::hosts_config::MirrorHost;

// Exports
mod config_response;
pub use config_response::ConfigResponse;

/// Struct for query parameters related to mirror config,
///
#[derive(Serialize, Deserialize)]
pub struct ConfigRequest {
    /// Namespace of the registry,
    ///
    ns: String,
    /// Stream format to configure,
    ///
    stream_format: Option<String>,
}

/// Handler for /config requests
/// 
#[handler]
pub async fn handle_config(
    method: Method,
    query: Query<ConfigRequest>,
    context: Data<&ThunkContext>,
) -> Result<ConfigResponse, Error> {
    _handle_config(method, query, context).await
}

/// Handler impl, seperated to test
/// 
async fn _handle_config(
    method: Method,
    Query(ConfigRequest { ns, stream_format }): Query<ConfigRequest>,
    context: Data<&ThunkContext>,
) -> Result<ConfigResponse, Error> {
    let app_host = context
        .search()
        .find_symbol("app_host")
        .unwrap_or("http://localhost:8578".to_string());

    let mirror_hosts_config = MirrorHost::get_hosts_config(&ns, app_host, true, stream_format);

    match method {
        Method::GET => {
            if mirror_hosts_config.installed(context.search().find_symbol("sysroot")) {
                Ok(ConfigResponse::ok())
            } else {
                Err(Error::recoverable_error("config is not installed"))
            }
        }
        Method::PUT => {
            info!("Configuring namespace {ns}");

            if let Err(err) = mirror_hosts_config.install(context.search().find_symbol("sysroot")) {
                error!("Unable to enable mirror host config for, {}, {:?}", ns, err);
                Err(Error::system_environment())
            } else {
                debug!("Enabled mirror host config for {}", ns);
                Ok(ConfigResponse::ok())
            }
        }
        Method::DELETE => {
            info!("Deleting config for namespace {ns}");
            if let Err(err) = mirror_hosts_config.uninstall(context.search().find_symbol("sysroot")) {
                error!("Unable to enable mirror host config for, {}, {:?}", ns, err);
                Err(Error::system_environment())
            } else {
                debug!("Enabled mirror host config for {}", ns);
                Ok(ConfigResponse::ok())
            }
        }
        _ => Err(Error::invalid_operation("unsupported method")),
    }
}

impl IntoResult<ConfigResponse> for Result<ConfigResponse, Error> {
    fn into_result(self) -> poem::Result<ConfigResponse> {
        match self {
            Ok(resp) => Ok(resp),
            Err(err) => {
                let resp = ConfigResponse::error(err);
                let resp = resp.into_response();

                Err(poem::Error::from_response(resp))
            }
        }
    }
}

#[allow(unused_imports)]
mod tests {
    use hyper::Method;
    use lifec::prelude::ThunkContext;
    use lifec::state::AttributeIndex;
    use poem::web::Data;
    use poem::web::Query;
    use poem::Endpoint;

    use crate::proxy::config::{ConfigRequest, _handle_config};

    #[tokio::test]
    async fn test_handler() {
        let _ = _handle_config(
            Method::GET,
            Query(ConfigRequest {
                ns: String::from("test.azurecr.io"),
                stream_format: None,
            }),
            Data(
                &ThunkContext::default()
                    .with_symbol("app_host", "test")
                    .with_symbol("sysroot", ".test_handle_config"),
            ),
        )
        .await
        .expect_err("should return an error");

        let _ = _handle_config(
            Method::PUT,
            Query(ConfigRequest {
                ns: String::from("test.azurecr.io"),
                stream_format: None,
            }),
            Data(
                &ThunkContext::default()
                    .with_symbol("app_host", "test")
                    .with_symbol("sysroot", ".test_handle_config"),
            ),
        )
        .await
        .expect("should put a config");

        let _ = _handle_config(
            Method::DELETE,
            Query(ConfigRequest {
                ns: String::from("test.azurecr.io"),
                stream_format: None,
            }),
            Data(
                &ThunkContext::default()
                    .with_symbol("app_host", "test")
                    .with_symbol("sysroot", ".test_handle_config"),
            ),
        )
        .await
        .expect("should put a config");
    }
}
