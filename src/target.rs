use http::uri::InvalidUri;
use hyper::{Method as HyperMethod, Uri};
use rand::seq::SliceRandom;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

#[derive(Debug, Copy, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub enum Method {
    #[serde(rename = "get")] Get,
    #[serde(rename = "put")] Put,
    #[serde(rename = "post")] Post,
    #[serde(rename = "delete")] Delete,
}

impl FromStr for Method {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "GET" | "get" | "Get" => Ok(Method::Get),
            "PUT" | "put" | "Put" => Ok(Method::Put),
            "POST" | "post" | "Post" => Ok(Method::Post),
            "DELETE" | "delete" | "Delete" => Ok(Method::Delete),
            _ => Err("Invalid method"),
        }
    }
}

impl From<Method> for HyperMethod {
    fn from(m: Method) -> Self {
        match m {
            Method::Get => HyperMethod::GET,
            Method::Put => HyperMethod::PUT,
            Method::Post => HyperMethod::POST,
            Method::Delete => HyperMethod::DELETE,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct TestTarget {
    /// HTTP endpoint relative to the URI
    pub endpoint: String,
    /// HTTP method
    pub method: Method,
    /// The methods to randomly test
    pub args: Vec<TestArg>,
}

impl TestTarget {
    pub fn method(&self) -> HyperMethod {
        self.method.into()
    }

    /// Randomly build an HTTP request in order to test this target.
    pub fn sample<R>(&self, base_url: &str, rng: &mut R) -> Result<Uri, InvalidUri>
    where
        R: Rng,
    {
        use TestArg::*;
        let mut uri = base_url.to_string();
        if !self.endpoint.starts_with("/") {
            uri.push('/');
        }
        uri.push_str(&self.endpoint);
        let mut qs = String::new();
        for arg in &self.args {
            match arg {
                Path { generator } => {
                    uri.push('/');
                    uri.push_str(&generator.sample(rng));
                }
                QueryString { name, value } => {
                    if qs.is_empty() {
                        qs.push('?');
                    } else {
                        qs.push('&');
                    }
                    qs.push_str(&name.sample(rng));
                    let val = value.sample(rng);
                    if !val.is_empty() {
                        qs.push('=');
                        qs.push_str(&val);
                    }
                }
            }
        }
        format!("{}{}", uri, qs).parse()
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "type")]
pub enum TestArg {
    /// relative path of the URI
    #[serde(rename = "path")]
    Path {
        #[serde(default)]
        generator: ArgGenerator
    },
    /// query string component
    #[serde(rename = "query")]
    QueryString {
        #[serde(default)]
        name: ArgGenerator,
        #[serde(default)]
        value: ArgGenerator,
    },
}

/// The criterion of argument generation
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "type")]
pub enum ArgGenerator<V = String> {
    /// Always provide the given string
    #[serde(rename = "fixed")]
    Fixed { value: V },
    /// Choose one of the given arguments at random
    #[serde(rename = "choice")]
    Choice { values: Vec<V> },
    /// Choose a random number from the given range
    #[serde(rename = "range")]
    IntRange { low: i64, high: i64 },
    /// Build a numeric sequence with the given length
    #[serde(rename = "numeric")]
    Numeric { len: u32 },
    /// Build an alphanumeric sequence with the given length
    #[serde(rename = "alphanumeric")]
    AlphaNumeric { len: u32 },
    /// Choose one of the given generators at random (OR)
    #[serde(rename = "union")]
    Union { generators: Vec<ArgGenerator<V>> },
    /// Generic "try multiple random things", easy to use
    #[serde(rename = "magic")]
    Magic,
}

/// The default is magic :)
impl<V> Default for ArgGenerator<V> {
    fn default() -> Self {
        ArgGenerator::Magic
    }
}

impl<V> ArgGenerator<V>
where
    V: std::fmt::Display,
{
    /// Randomly sample a value for use in the test.
    pub fn sample<R>(&self, rng: &mut R) -> String
    where
        R: Rng,
    {
        use ArgGenerator::*;
        match self {
            Fixed { value } => value.to_string(),
            Choice { values } if values.is_empty() => "".to_string(),
            Union { generators } if generators.is_empty() => "".to_string(),
            Union { generators } => generators.choose(rng).unwrap().sample(rng),
            Choice { values } => values.choose(rng).unwrap().to_string(),
            IntRange { low, high } => rng.gen_range(*low ..= *high).to_string(),
            Numeric { len } => std::iter::repeat_with(|| rng.gen_range(b'0' ..= b'9') as char)
                .take(*len as usize)
                .collect(),
            AlphaNumeric { len } => std::iter::repeat_with(|| {
                rng.sample(rand::distributions::Alphanumeric)
            }).take(*len as usize)
                .map(|c| c as char)
                .collect(),
            Magic => {
                let generators = [
                    Choice { values: vec!["", "false", "true", "null", "undefined", "NaN", "%20", "%27"] },
                    AlphaNumeric { len: 16 },
                    IntRange { low: -100000, high: 100000 },
                ];

                generators.choose(rng)
                    .expect("There should be at least one generator")
                    .sample(rng)
            }
        }
    }
}
