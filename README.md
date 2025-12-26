# ecmwf-opendata (Rust)

Rust client for ECMWF Open Data.

API documentation (docs.rs): https://docs.rs/ecmwf-opendata

This crate is a Rust re-implementation of the *core* functionality of the upstream Python project `ecmwf-opendata`:

- Build **MARS-like requests** (keyword/value pairs)
- Resolve request → **data URLs**
- Download whole files, or download selected fields via the `.index` sidecar using HTTP `Range` requests

Chinese documentation: see [README.zh-CN.md](README.zh-CN.md).

## Install

```bash
cargo add ecmwf-opendata
```

## Library usage

### 1) Python-like `Client(...)` options

Python:

```python
client = Client(
    source="ecmwf",
    model="ifs",
    resol="0p25",
    preserve_request_order=False,
    infer_stream_keyword=True,
)
```

Rust:

```rust
use ecmwf_opendata::{Client, ClientOptions};

let opts = ClientOptions {
    source: "ecmwf".to_string(),
    model: "ifs".to_string(),
    resol: "0p25".to_string(),
    preserve_request_order: false,
    infer_stream_keyword: true,
    ..ClientOptions::default()
};

let client = Client::new(opts)?;
# Ok::<(), ecmwf_opendata::Error>(())
```

`source` can be a known mirror (`"ecmwf"`, `"aws"`, `"azure"`, `"google"`) or a custom base URL (`"https://..."`).

### 2) Request builder (kwargs-ish)

```rust
use ecmwf_opendata::{Client, ClientOptions, Request};

let client = Client::new(ClientOptions::default())?;

let req = Request::new()
    .r#type("fc")
    .param("msl")
    .step(240)
    .target("data.grib2");

let result = client.retrieve_request(req)?;
println!("Downloaded {} bytes", result.size_bytes);
# Ok::<(), ecmwf_opendata::Error>(())
```

### 3) `retrieve_pairs`: strongest “kwargs/dict” feel

```rust
use ecmwf_opendata::{Client, ClientOptions};

let client = Client::new(ClientOptions::default())?;

let result = client.retrieve_pairs([
    ("type", "fc".into()),
    ("param", "msl".into()),
    ("step", 240.into()),
    ("target", "data.grib2".into()),
])?;

println!("{}", result.datetime);
# Ok::<(), ecmwf_opendata::Error>(())
```

## CLI

This repository also includes a small CLI example.

```bash
cargo run -- retrieve data.grib2
cargo run -- download data.grib2
```

## Notes / limitations

- This is intentionally a “core features” port; it does not aim to fully replicate every upstream Python feature.
- `latest()` probing depends on endpoint availability. If it fails, specify `date`/`time` explicitly.
- Data usage is subject to ECMWF Open Data terms (including attribution requirements).
