use std::{fmt::Display, string};

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
        Error {
            category: ErrorCategory::InvalidOperation(reason),
        }
    }

    /// This operation is expected to fail at times and the runtime is expected to recover from it,
    ///
    pub fn recoverable_error(details: &'static str) -> Self {
        warn!("Recoverable error, {details}");
        Error {
            category: ErrorCategory::RecoverableError(details),
        }
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

    /// Returns an error that indicates a coding error,
    /// 
    pub fn code_defect() -> Self {
        Error {
            category: ErrorCategory::CodeDefect
        }
    }

    /// Returns true if the category is recoverable,
    /// 
    pub fn is_recoverable(&self) -> bool {
        match self.category { 
            ErrorCategory::RecoverableError(_) => true,
            _ => false, 
        }
    }

    /// Returns a composite error,
    ///
    pub fn also(&self, other: Self) -> Self {
        Self {
            category: ErrorCategory::Composite(
                Box::new(self.category.clone()),
                Box::new(other.category),
            ),
        }
    }

    /// Returns the current error category,
    /// 
    pub fn category(&self) -> &ErrorCategory {
       &self.category
    }
}

#[derive(Debug, Clone)]
pub enum ErrorCategory {
    Authentication,
    DataFormat,
    ExternalDependency,
    ExternalDependencyWithStatusCode(StatusCode),
    SystemEnvironment,
    CodeDefect,
    InvalidOperation(&'static str),
    RecoverableError(&'static str),
    Composite(Box<Self>, Box<Self>),
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

impl From<tokio::sync::oneshot::error::RecvError> for Error {
    fn from(value: tokio::sync::oneshot::error::RecvError) -> Self {
        error!("Error receiving result from a oneshot channel, likely a code-defect {value}");
        Self::code_defect()
    }
}

impl From<string::FromUtf8Error> for Error {
    fn from(value: string::FromUtf8Error) -> Self {
        error!("Error converting from bytes to string, input was not utf8, {value}");
        Self::data_format()
    }
}

impl From<base64_url::base64::DecodeError> for Error {
    fn from(value: base64_url::base64::DecodeError) -> Self {
        error!("Error decoding base64 string, input was not base64, {value}");
        Self::data_format()
    }
}

impl From<std::time::SystemTimeError> for Error {
    fn from(value: std::time::SystemTimeError) -> Self {
        error!("Could not convert system time to duration, {value}");
        Self::code_defect()
    }
}

impl From<Error> for lifec::error::Error {
    fn from(value: Error) -> lifec::error::Error {
        match &value.category {
            ErrorCategory::Authentication => lifec::error::Error::invalid_operation("authentication failure"),
            ErrorCategory::DataFormat => lifec::error::Error::invalid_operation("invalid data format"),
            ErrorCategory::ExternalDependency => lifec::error::Error::invalid_operation("external dependency failure"),
            ErrorCategory::ExternalDependencyWithStatusCode(status_code) => {
                if let Some(reason) = status_code.canonical_reason() {
                    lifec::error::Error::invalid_operation(reason)
                } else {
                    lifec::error::Error::invalid_operation("http error")
                }
            },
            ErrorCategory::CodeDefect => lifec::error::Error::invalid_operation("code defect"),
            ErrorCategory::SystemEnvironment => lifec::error::Error::invalid_operation("system environment error"),
            ErrorCategory::InvalidOperation(reason) => lifec::error::Error::invalid_operation(reason),
            ErrorCategory::RecoverableError(message) if message.starts_with("skip") => lifec::error::Error::skip(message),
            ErrorCategory::RecoverableError(message) => lifec::error::Error::recoverable(message),
            ErrorCategory::Composite(a, b) => match (*a.clone(), *b.clone()) {
                (ErrorCategory::RecoverableError(message), _) | (_, ErrorCategory::RecoverableError(message)) if message.starts_with("skip") => lifec::error::Error::skip(message),
                (ErrorCategory::RecoverableError(message), _) | (_, ErrorCategory::RecoverableError(message)) => lifec::error::Error::recoverable(message),
                (a, b) => {
                    error!("Composite error, {:?} + {:?}", a, b);
                    lifec::error::Error::invalid_operation("composite error")
                }
            },
        }
    }
}

#[allow(unused_imports)]
mod tests {
    use crate::Error;

    #[test]
    fn test_is_recoverable() {
        let e = Error::recoverable_error("test");

        assert!(e.is_recoverable());
    }
}

impl From<toml_edit::TomlError> for Error {
    fn from(value: toml_edit::TomlError) -> Self {
        error!("Error parsing toml, {value}");
        Self::data_format().also(Self::recoverable_error("Can output correct toml"))
    }
}
