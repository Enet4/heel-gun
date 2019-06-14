# Heel Gun
[![Build Status](https://travis-ci.org/Enet4/heel-gun.svg?branch=master)](https://travis-ci.org/Enet4/heel-gun) [![dependency status](https://deps.rs/repo/github/Enet4/heel-gun/status.svg)](https://deps.rs/repo/github/Enet4/heel-gun) ![Minimum Rust Version Stable](https://img.shields.io/badge/rustc-stable-green.svg)

Test your HTTP server for robustness to arbitrary inputs. `heel-gun` is a tool
which performs several HTTP requests to identify cases where the server
misbehaves. Requests are built randomly based on a set of configurable rules.

## Using

This CLI tool expects two main arguments: the base URL to the HTTP server, and
a configuration file defining the HTTP endpoints to test and how these
arguments are generated.

```none
USAGE:
    heel-gun [OPTIONS] <url> <config> [outdir]

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
    -N <n>        number of iterations to test for each target [default: 100]

ARGS:
    <url>       the base URL to test
    <config>    path to configuration file
    <outdir>    path to the output directory containing the logs [default: output]
```

Example:

```
heel-gun http://testmachine.myspot.net:8080 resources/example.yaml -N 4
```

This will test the server with a random assortment of requests, such as these:

```none
GET http://testmachine.myspot.net:8080/cool-endpoint/lBtY2g18?id=0&more=891134
GET http://testmachine.myspot.net:8080/cool-endpoint/ie9EMV9G?id=-1&more=238164
GET http://testmachine.myspot.net:8080/cool-endpoint/dJ7iV7cs?id=null&more=415128
GET http://testmachine.myspot.net:8080/cool-endpoint/HCvpC90k?id=null&more=902781
POST http://testmachine.myspot.net:8080/user/UBwqFvFnXh?admin=undefined
POST http://testmachine.myspot.net:8080/user/LkspwEu0g4?admin=null
POST http://testmachine.myspot.net:8080/user/pkgagTBnem?admin
POST http://testmachine.myspot.net:8080/user/rRdlgzll2D?admin=false
```

And record problematic responses in a CSV file:

```csv
method,uri,reason
GET,http://testmachine.myspot.net:8080/cool-endpoint/lBtY2g18?id=0&more=891134,501 Not Implemented
GET,http://testmachine.myspot.net:8080/cool-endpoint/ie9EMV9G?id=-1&more=238164,501 Not Implemented
GET,http://testmachine.myspot.net:8080/cool-endpoint/dJ7iV7cs?id=null&more=415128,501 Not Implemented
GET,http://testmachine.myspot.net:8080/cool-endpoint/HCvpC90k?id=null&more=902781,501 Not Implemented
POST,http://testmachine.myspot.net:8080/user/UBwqFvFnXh?admin=undefined,501 Not Implemented
POST,http://testmachine.myspot.net:8080/user/LkspwEu0g4?admin=null,501 Not Implemented
POST,http://testmachine.myspot.net:8080/user/pkgagTBnem?admin,501 Not Implemented
POST,http://testmachine.myspot.net:8080/user/rRdlgzll2D?admin=false,501 Not Implemented
```

Moreover, the HTTP bodies of server responses are saved in an output directory:

```none
output/
├── GET
│   └── cool-endpoint
│       ├── lBtY2g18?id=0&more=891134
│       ├── ie9EMV9G?id=-1&more=238164
│       ├── dJ7iV7cs?id=null&more=415128
│       └──  HCvpC90k?id=null&more=902781
└── POST
    └── user
        ├── UBwqFvFnXh?admin=undefined
        ├── LkspwEu0g4?admin=null
        ├── pkgagTBnem?admin
        └── rRdlgzll2D?admin=false
```


For the time being, problematic responses are either HTTP responses with a
`5xx` status code, or requests which result in a broken or timed out connection.

`<config>` is a file describing a set of rules for producing URI paths and
other parameters such as query string arguments.
A more extensive documentation will be provided eventually. In the mean time,
please see the [resources](resources) directory for examples.

You can also define the `RUST_LOG` environment variable for additional logging
output (as defined by [`log`](https://crates.io/crates/log), to one of "error",
"warn", "info", "debug" or "trace"):

```
RUST_LOG=info heel-gun http://testmachine.myspot.net:8080 resources/example.yaml
```

## License and Warning Note

Licensed under either of

* Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
* MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any
additional terms or conditions.

In spite of the main goal of testing for server robustness, this tool may also
present itself as capable of doing dangerous mistakes (such as running in
production), poorly intended actions (DoS attacks), and other sorts of misuse.
Please be responsible when using `heel-gun`. As defined by the aforementioned
license, all authors and contributors to `heel-gun` cannot be held liable for
any damage which may occur from the use of this software.
