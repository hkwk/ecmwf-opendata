#![forbid(unsafe_code)]

//! Rust client for ECMWF Open Data.
//!
//! This crate is a Rust re-implementation of the core ideas from the upstream
//! `ecmwf-opendata` Python package: you express a MARS-like request (keyword/value
//! pairs), URLs are derived from that request, and downloads can either fetch whole
//! files or use the `.index` sidecar to download only matching fields via HTTP
//! range requests.
//!
//! **Quick start**
//! ```no_run
//! use ecmwf_opendata::{Client, ClientOptions, Request};
//!
//! let opts = ClientOptions {
//!     // Python: Client(source="ecmwf", model="ifs", resol="0p25", ...)
//!     source: "ecmwf".to_string(),
//!     model: "ifs".to_string(),
//!     resol: "0p25".to_string(),
//!     preserve_request_order: false,
//!     infer_stream_keyword: true,
//!     ..ClientOptions::default()
//! };
//! let client = Client::new(opts)?;
//!
//! // Builder style
//! let req = Request::new().r#type("fc").param("msl").step(240).target("data.grib2");
//! let result = client.retrieve_request(req)?;
//! println!("{} bytes", result.size_bytes);
//! # Ok::<(), ecmwf_opendata::Error>(())
//! ```
//!
//! **Pairs (kwargs-like) style**
//! ```no_run
//! use ecmwf_opendata::{Client, ClientOptions, RequestValue};
//!
//! let client = Client::new(ClientOptions::default())?;
//! let result = client.retrieve_pairs([
//!     ("type", RequestValue::from("fc")),
//!     ("param", RequestValue::from("msl")),
//!     ("step", 240.into()),
//!     ("target", "data.grib2".into()),
//! ])?;
//! println!("{}", result.datetime);
//! # Ok::<(), ecmwf_opendata::Error>(())
//! ```
//!
//! Notes:
//! - Downloads are governed by ECMWF Open Data terms (e.g. attribution requirements).
//! - Network conditions vary by mirror/source; if `latest()` cannot be established,
//!   specify `date`/`time` explicitly in your request.

mod client;
mod date;
mod error;
mod request;
mod sources;
mod url_builder;

pub use crate::client::{Client, ClientOptions, Result};
pub use crate::error::{Error, Result as EResult};
pub use crate::request::{Request, RequestValue};
