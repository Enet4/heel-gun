#![deny(unsafe_code)]

use std::io::Error as IoError;
use std::fs::{create_dir_all, File};
use std::path::PathBuf;
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
use log::{info, warn, error};
use structopt::StructOpt;
use tokio::runtime::Runtime;
use tokio_io::AsyncWrite;

mod config;
use config::Config;
mod outcome;
use outcome::*;
mod target;
use target::*;

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
    /// some error occurred while fetching a response body
    #[fail(display = "Hyper error: {}", err)]
    Hyper {
        err: HyperError,
    },
    /// some error occurred while doing disk I/O 
    #[fail(display = "I/O error: {}", err)]
    Io {
        err: std::io::Error,
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

impl From<IoError> for Error {
    fn from(err: IoError) -> Self {
        Error::Io { err }
    }
}

impl From<HyperError> for Error {
    fn from(err: HyperError) -> Self {
        Error::Hyper { err }
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
                        (i, target.clone(), uri)
                    })
                    .map_err(|e| e.into()),
            )
        })
        // build HTTP request
        .and_then(move |(i, target, uri)| {
            let method = target.method();
            result(
                match Request::builder()
                    .method(target.method())
                    .uri(&uri)
                    .body(Body::empty())
                {
                    Ok(req) => Ok((i, target, uri, req)),
                    Err(err) => Err(Error::Http { method, uri, err }),
                },
            )
        })
        // send request
        .and_then(move |(_i, target, uri, req)| {
            let method = target.method();
            client.request(req).then(move |r| match r {
                Ok(r) => {
                    // convert 5xx server responses to errors
                    let status = r.status();
                    let body = r.into_body();
                    if status.is_server_error() {
                        warn!("{:?} {:?} -> returned error {}", method, uri, status);
                    } else {
                        info!("Response: {}", status);
                    }
                    Ok(ServerOutcome::with_status(method, uri, status, body))
                }
                Err(err) => {
                    if err.is_connect() {
                        Err(Error::HttpStream { method, uri, err })
                    } else {
                        // errors that are the server's fault should stick to ServerOutcome
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
    let executor = runtime.executor();
    runtime.block_on(
            iter_ok::<_, Error>(targets)
                .map(move |target| test_target_requests(client.clone(), url.to_string(), target, n))
                .flatten()
                // write errors to failure record
                .and_then(move |outcome| match outcome.kind {
                    OutcomeKind::BadError { status, body } => {
                        let method = outcome.method.to_string();
                        let uri = outcome.uri.to_string();
                        let reason = status.to_string();

                        // write body to independent file
                        let trimmed_uri = outcome.uri.path_and_query().unwrap().to_string();
                        let body_path = outdir.join(format!("{}/{}", method, trimmed_uri));
                        let body_path_parent = body_path.parent().unwrap().to_owned();
                        info!("\tSaving response body to {}", body_path.display());
                        let report_file = tokio_fs::create_dir_all(body_path_parent)
                            .and_then(|_| tokio_fs::File::create(body_path))
                            .map_err(Error::from)
                            .and_then(move |mut file| {
                                body.map_err(Error::from)
                                    .for_each(move |chunk| {
                                        result(file.poll_write(&chunk).map(|_|()).map_err(Error::from))
                                    })
                            }).map_err(|e| {
                                error!("Could not save response: {}", e);
                                ()
                            });
                        executor.spawn(report_file);
                        //runtime.spawn(report_file.map_err(|_|()));

                        result(
                            // write record to CSV file
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
                .for_each(|_| ok(()))
                .map_err(|e| {
                    error!("Irrecoverable error occurred.");
                    error!("\t{}", e);
                    error!("Server test stopped abruptly.");
                })
        )
        .unwrap();
    println!("Failure log recorded in {}", output_filename.display());
    runtime.shutdown_now().wait().unwrap();
}
