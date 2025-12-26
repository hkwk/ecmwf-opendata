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

### 4）宏：最接近 Python `client.retrieve(time=..., ...)`

```rust
use ecmwf_opendata::{Client, ClientOptions, retrieve};

let client = Client::new(ClientOptions::default())?;

let steps: Vec<i32> = (12..=360).step_by(12).collect();

let result = retrieve!(
    client,
    time = 0,
    stream = "enfo",
    type = "ep",
    step = steps,
    levelist = 850,
    param = [
        "ptsa_gt_1stdev",
        "ptsa_gt_1p5stdev",
        "ptsa_gt_2stdev",
        "ptsa_lt_1stdev",
        "ptsa_lt_1p5stdev",
        "ptsa_lt_2stdev",
    ],
    target = "data.grib2",
)?;

println!("{}", result.datetime);
# Ok::<(), ecmwf_opendata::Error>(())
```

### 5）GUI/配置场景：字符串 key/value 输入

如果你的 UI（或配置文件）把值都当作字符串保存，可以用 `Request::from_str_pairs` 来构造 request。

```rust
use ecmwf_opendata::{Client, ClientOptions, Request};

let client = Client::new(ClientOptions::default())?;

// 示例：值来自文本框
let req = Request::from_str_pairs([
    ("time", "0"),
    ("stream", "enfo"),
    ("type", "ep"),
    ("step", "12,24,36"),
    ("levelist", "850"),
    ("param", "tpg1,tpg5,10fgg10"),
    ("target", "data.grib2"),
]);

let result = client.retrieve_request(req)?;
println!("{}", result.datetime);
# Ok::<(), ecmwf_opendata::Error>(())
```

## CLI

仓库里也包含一个简单 CLI 示例：

```bash
cargo run --example cli -- retrieve data.grib2
cargo run --example cli -- download data.grib2
```

## 说明 / 限制

- 这是「核心功能」移植，暂不追求 100% 复刻上游 Python 的所有细节。
- `latest()` 探测会受镜像/网络影响；若失败，建议在 request 中显式指定 `date`/`time`。
- 数据使用需遵守 ECMWF Open Data 条款（包括署名/引用等要求）。
