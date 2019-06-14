use itertools::Itertools;
use serde::Deserialize;
use serde_json;
use serde_yaml;
use std::ffi::OsStr;
use std::fs::{read_to_string, File};
use std::path::Path;

use crate::target::{ArgGenerator, TestArg, TestTarget};

pub type DynError = Box<std::error::Error + Send + Sync>;

/// Server testing configuration object
#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub targets: Vec<TestTarget>,
}

impl Config {
    pub fn from_file<P: AsRef<Path>>(filepath: P) -> Result<Self, DynError> {
        let path = filepath.as_ref();
        if path.file_name() == Some(OsStr::new("routes")) {
            return Config::from_routes_file(path);
        }
        let config = match path.extension().and_then(|ext| ext.to_str()) {
            Some("json") => serde_json::from_reader(File::open(path)?)?,
            Some("yml") | Some("yaml") => serde_yaml::from_reader(File::open(path)?)?,
            _ => Err("Unsupported configuration file: must be .json .yml)")?,
        };
        Ok(config)
    }

    /// Build a configuration based on a Play Framework (v2) "routes" file
    pub fn from_routes_file<P: AsRef<Path>>(filepath: P) -> Result<Self, DynError> {
        let text = read_to_string(filepath.as_ref())?;

        let targets: Vec<_> = text
            .lines()
            .map(|t| {
                if let Some(i) = t.find('#') {
                    &t[..i]
                } else {
                    t
                }
            })
            .filter(|l| !l.is_empty())
            .filter_map(|l| match l.split_whitespace().next_tuple() {
                t @ Some(("*", _))
                | t @ Some(("GET", _))
                | t @ Some(("POST", _))
                | t @ Some(("PUT", _))
                | t @ Some(("DELETE", _)) => t,
                _ => None,
            })
            .flat_map(|(method, uri)| {
                if method == "*" {
                    vec![("GET", uri), ("POST", uri), ("PUT", uri), ("DELETE", uri)]
                } else {
                    vec![(method, uri)]
                }
            })
            .map(|(method, uri)| {
                let (endpoint, args) = Config::parse_route_uri(uri)?;
                Ok(TestTarget {
                    endpoint,
                    method: method.parse()?,
                    args,
                })
            })
            .flat_map(|r: Result<_, DynError>| {
                if let Err(e) = &r {
                    eprintln!("{}", e);
                    eprintln!("Ignoring route due to the previous error.");
                }

                r
            })
            .collect();

        Ok(Config { targets })
    }

    fn parse_route_uri(uri: &str) -> Result<(String, Vec<TestArg>), DynError> {
        let mut base_endpoint = String::with_capacity(uri.len());
        let mut args = Vec::new();
        let mut has_param = false;
        for component in uri.split('/') {
            if component.contains('*') {
                return Err(format!(
                    "could not read URI '{}': routes with wildcard '*' are currently not supported",
                    uri
                )
                .into());
            }
            if component.starts_with(":") {
                // component parameter
                args.push(TestArg::Path {
                    generator: ArgGenerator::Magic,
                });
                has_param = true;
            } else if !has_param {
                if !base_endpoint.is_empty() {
                    base_endpoint.push('/');
                }
                base_endpoint.push_str(component);
            } else {
                args.push(TestArg::Path {
                    generator: ArgGenerator::Fixed {
                        value: component.to_owned(),
                    },
                })
            }
        }

        Ok((base_endpoint, args))
    }
}
