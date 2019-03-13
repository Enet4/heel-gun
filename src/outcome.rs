use http::uri::Uri;
use http::StatusCode;
use hyper::error::Error as HyperError;
use hyper::Method;

/// The outcome of a single HTTP request to the server. It either represents a
/// "good" outcome (a reasonable response is obtained from the server), or
/// a "bad" outcome (the server responded with a server error or the connection
/// was cut off).
#[derive(Debug)]
pub struct ServerOutcome {
    /// the HTTP method of the request performed
    pub method: Method,
    /// the URI of the HTTP request performed
    pub uri: Uri,
    /// the kind of outcome
    pub kind: OutcomeKind,
}

impl ServerOutcome {
    pub fn with_status(method: Method, uri: Uri, status: StatusCode) -> Self {
        ServerOutcome {
            method,
            uri,
            kind: if status.is_server_error() {
                OutcomeKind::BadError { status }
            } else {
                OutcomeKind::Good { status }
            },
        }
    }

    pub fn bad_http(method: Method, uri: Uri, err: HyperError) -> Self {
        ServerOutcome {
            method,
            uri,
            kind: OutcomeKind::BadHttp { err },
        }
    }
}

#[derive(Debug)]
/// Value differentiating the kind of server test outcome and
/// providing kind-specific information
pub enum OutcomeKind {
    /// Good!
    Good {
        /// the status code returned by the server
        status: StatusCode,
    },
    /// The server returned a server error response (bad!)
    BadError {
        /// the status code returned by the server (sure to be 5xx)
        status: StatusCode,
    },
    /// An error emerged at the HTTP layer (bad!)
    BadHttp { err: HyperError },
}
