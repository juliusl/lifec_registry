use std::fmt::Display;

use hyper::{http::uri::InvalidUri, StatusCode};
use tracing::{error, warn};

/// Struct to represent when the library encounters an error, 
/// 
#[allow(dead_code)]
#[derive(Debug)]
pub struct Error {
    category: ErrorCategory,
}

impl Error {
    /// There was insuffecient data to complete a process
    ///
    pub fn invalid_operation(reason: &'static str) -> Self {
        error!("Error while executing an operation, reason: {reason}");
        Error { category: ErrorCategory::InvalidOperation(reason) }
    }

    /// This operation is expected to fail at times and the runtime is expected to recover from it,
    /// 
    pub fn recoverable_error(details: &'static str) -> Self {
        warn!("Recoverable error, {details}");
        Error { category: ErrorCategory::RecoverableError(details) }
    }

    /// Returns an error that indicates a data-format issue,
    ///
    pub fn data_format() -> Self {
        Error {
            category: ErrorCategory::DataFormat,
        }
    }

    /// Returns an error that indicates that there was an authentication issue,
    ///
    pub fn authentication() -> Self {
        Error {
            category: ErrorCategory::Authentication,
        }
    }

    /// Returns an error that indicates that there was an error using an external dependency,
    ///
    pub fn external_dependency() -> Self {
        Error {
            category: ErrorCategory::ExternalDependency,
        }
    }

    /// Returns an error that indicates that there was an error using an external dependency w/ a status code,
    /// 
    pub fn external_dependency_with(status_code: StatusCode) -> Self {
        Error {
            category: ErrorCategory::ExternalDependencyWithStatusCode(status_code),
        }
    }

    /// Returns an error that indicates that there was an error with the system env. For example reading a file, etc.
    ///
    pub fn system_environment() -> Self {
        Error {
            category: ErrorCategory::SystemEnvironment,
        }
    }
}

#[derive(Debug)]
enum ErrorCategory {
    Authentication,
    DataFormat,
    ExternalDependency,
    ExternalDependencyWithStatusCode(StatusCode),
    SystemEnvironment,
    InvalidOperation(&'static str),
    RecoverableError(&'static str),
}

impl std::error::Error for Error {}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl From<InvalidUri> for Error {
    fn from(value: InvalidUri) -> Self {
        error!("Error parsing uri, {value}");
        Self::data_format()
    }
}

impl From<hyper::Error> for Error {
    fn from(value: hyper::Error) -> Self {
        error!("Error making http request, {value}");
        Self::external_dependency()
    }
}

impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        error!("Error w/ system i/o, {value}");
        Self::system_environment()
    }
}

impl From<serde_json::Error> for Error {
    fn from(value: serde_json::Error) -> Self {
        error!("Error with json serialization, {value}");
        Self::data_format()
    }
}

impl From<azure_core::Error> for Error {
    fn from(value: azure_core::Error) -> Self {
        error!("Error with azure sdk, {value}");
        Self::external_dependency()
    }
}

impl From<hyper::http::Error> for Error {
    fn from(value: hyper::http::Error) -> Self {
        error!("Error making http request, {value}");
        Self::external_dependency()
    }
}