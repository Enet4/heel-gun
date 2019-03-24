#![deny(unsafe_code)]

use std::fs::{create_dir_all, File};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use env_logger;
use failure::Fail;
use futures::future::{ok, result};
use futures::prelude::*;
use futures::stream::iter_ok;
use http::uri::{InvalidUri, Uri};
use http::Error as HttpError;
use hyper::error::Error as HyperError;
use hyper::{Body, Client, Method, Request};
use log::{info, warn};
use serde::Deserialize;
use serde_json;
use serde_yaml;
use structopt::StructOpt;
use tokio::runtime::Runtime;

mod outcome;
use outcome::*;
mod target;
use target::*;

type DynError = Box<std::error::Error + Send + Sync>;

/// Test for HTTP server robustness
#[derive(Debug, StructOpt)]
pub struct HeelGun {
    /// the base URL to test
    url: String,
    /// path to configuration file
    #[structopt(parse(from_os_str))]
    config: PathBuf,
    /// number of iterations to test for each target
    #[structopt(short = "N", default_value = "100")]
    n: u32,
    /// path to the output directory containing the logs
    #[structopt(parse(from_os_str), default_value = "output")]
    outdir: PathBuf,
}

/// Server testing configuration object
#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    targets: Vec<TestTarget>,
}

impl Config {
    pub fn from_file<P: AsRef<Path>>(filepath: P) -> Result<Self, DynError> {
        let path = filepath.as_ref();
        let config = match path.extension().and_then(|ext| ext.to_str()) {
            Some("json") => serde_json::from_reader(File::open(path)?)?,
            Some("yml") | Some("yaml") => serde_yaml::from_reader(File::open(path)?)?,
            _ => Err("Unsupported configuration file extension (must be .json or .yml)")?,
        };
        Ok(config)
    }
}

/// Errors obtained from target testing
#[derive(Debug, Fail)]
pub enum Error {
    /// some other error occurred while handling the HTTP stream
    #[fail(display = "HTTP stream error: {}", err)]
    HttpStream {
        /// the HTTP method of the request performed
        method: Method,
        /// the URI of the HTTP request performed
        uri: Uri,
        err: HyperError,
    },
    /// some other error occurred while building HTTP content
    #[fail(display = "HTTP error: {}", err)]
    Http {
        /// the HTTP method of the request performed
        method: Method,
        /// the URI of the HTTP request performed
        uri: Uri,
        err: HttpError,
    },
    /// the target test sampler produced an illegal URI
    #[fail(display = "Invalid request URI: {}", err)]
    InvalidRequest {
        #[fail(cause)]
        err: InvalidUri,
    },
    /// could not write a failure entry to disk
    #[fail(display = "Failed to write outcome: {}", err)]
    WriteFailure {
        #[fail(cause)]
        err: csv::Error,
    },
}

impl From<InvalidUri> for Error {
    fn from(err: InvalidUri) -> Self {
        Error::InvalidRequest { err }
    }
}

impl From<csv::Error> for Error {
    fn from(err: csv::Error) -> Self {
        Error::WriteFailure { err }
    }
}

/// Obtain a stream of requests and respective responses from a test target.
fn test_target_requests<C: 'static, U: 'static>(
    client: Arc<Client<C>>,
    base_url: U,
    target: TestTarget,
    niter: u32,
) -> impl Stream<Item = ServerOutcome, Error = Error> + 'static
where
    C: hyper::client::connect::Connect,
    U: AsRef<str>,
{
    let mut rng = rand_pcg::Pcg64Mcg::new(rand::random());

    // not required, but prevents deep copying of the test target object
    let target = Arc::from(target);

    iter_ok::<_, Error>(0..niter)
        // sample request URI
        .and_then(move |i| {
            result(
                target
                    .sample(base_url.as_ref(), &mut rng)
                    .map(|uri| {
                        info!("{:4} > {:?} {:?}", i, target.method(), uri);
                        (target.clone(), uri)
                    })
                    .map_err(|e| e.into()),
            )
        })
        // build HTTP request
        .and_then(move |(target, uri)| {
            let method = target.method();
            result(
                match Request::builder()
                    .method(target.method())
                    .uri(&uri)
                    .body(Body::empty())
                {
                    Ok(req) => Ok((target, uri, req)),
                    Err(err) => Err(Error::Http { method, uri, err }),
                },
            )
        })
        // send request
        .and_then(move |(target, uri, req)| {
            let method = target.method();
            client.request(req).then(move |r| match r {
                Ok(r) => {
                    // convert 5xx server responses to errors
                    let status = r.status();
                    if status.is_server_error() {
                        warn!("{:?} {:?} -> returned error {}", method, uri, status);
                    } else {
                        info!("Response: {}", status);
                    }
                    Ok(ServerOutcome::with_status(method, uri, status))
                }
                // !!! errors that are the server's fault should stick to ServerOutcome
                Err(err) => {
                    if err.is_connect() {
                        Err(Error::HttpStream { method, uri, err })
                    } else {
                        Ok(ServerOutcome::bad_http(method, uri, err))
                    }
                }
            })
        })
}

fn main() {
    env_logger::init();
    let HeelGun {
        config: config_file,
        n,
        url,
        outdir,
    } = HeelGun::from_args();

    let Config { targets } = Config::from_file(config_file).unwrap();

    create_dir_all(&outdir).unwrap();

    let client = Arc::new(Client::new());

    let mut runtime = Runtime::new().unwrap();
    let output_filename = outdir.join("failures.csv");
    let failures = File::create(&output_filename).unwrap();
    let mut failures = csv::Writer::from_writer(failures);
    failures.write_record(&["method", "uri", "reason"]).unwrap();
    runtime
        .block_on(
            iter_ok::<_, Error>(targets)
                .map(move |target| test_target_requests(client.clone(), url.to_string(), target, n))
                .flatten()
                // write errors to failure record
                .and_then(move |outcome| match outcome.kind {
                    OutcomeKind::BadError { status } => {
                        let method = outcome.method.to_string();
                        let uri = outcome.uri.to_string();
                        let reason = status.to_string();
                        result(
                            failures
                                .write_record(&[&method, &uri, &reason])
                                .map_err(|e| e.into()),
                        )
                    }
                    OutcomeKind::BadHttp { err } => {
                        let method = outcome.method.to_string();
                        let uri = outcome.uri.to_string();
                        let reason = err.to_string();
                        result(
                            failures
                                .write_record(&[&method, &uri, &reason])
                                .map_err(|e| e.into()),
                        )
                    }
                    OutcomeKind::Good { .. } => ok(()),
                })
                .for_each(|_| ok(())),
        )
        .unwrap_or_else(|e| {
            eprintln!("Irrecoverable error occurred.");
            eprintln!("\t{}", e);
            eprintln!("Server test stopped abruptly.");
        });
    println!("Failure log recorded in {}", output_filename.display());
    runtime.shutdown_now().wait().unwrap();
}
