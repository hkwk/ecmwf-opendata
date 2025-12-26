# ecmwf-opendata（Rust）

ECMWF Open Data 的 Rust 客户端。

API 文档（docs.rs）：https://docs.rs/ecmwf-opendata

本 crate 参考并对齐了上游 Python 项目 `ecmwf-opendata` 的**核心思路**：

- 用类似 MARS 的方式用「关键字/值」来表达 request
- request → 推导出数据文件 URL
- 支持整文件下载；也支持读取 `.index`（JSON lines）并基于 `_offset/_length` 做 HTTP `Range` 下载，从而只下载匹配字段

English documentation: see [README.md](README.md).

## 安装

```bash
cargo add ecmwf-opendata
```

## 作为库使用

### 1）Python `Client(...)` 的选项如何对应

Python：

```python
client = Client(
    source="ecmwf",
    model="ifs",
    resol="0p25",
    preserve_request_order=False,
    infer_stream_keyword=True,
)
```

Rust：

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

`source` 可以是内置镜像（`"ecmwf"` / `"aws"` / `"azure"` / `"google"`），也可以直接传自定义 base URL（`"https://..."`）。

### 2）Request builder（kwargs 风格）

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

### 3）`retrieve_pairs`：更像 Python dict/kwargs

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

仓库里也包含一个简单 CLI 示例：

```bash
cargo run -- retrieve data.grib2
cargo run -- download data.grib2
```

## 说明 / 限制

- 这是「核心功能」移植，暂不追求 100% 复刻上游 Python 的所有细节。
- `latest()` 探测会受镜像/网络影响；若失败，建议在 request 中显式指定 `date`/`time`。
- 数据使用需遵守 ECMWF Open Data 条款（包括署名/引用等要求）。

## 发布到 crates.io 的建议流程

建议补齐：

- 在 `Cargo.toml` 增加你的 `repository`（以及可选的 `homepage`）
- 然后运行：
  - `cargo test`
  - `cargo fmt`
  - `cargo clippy -- -D warnings`
  - `cargo package`
  - `cargo publish --dry-run`
