use std::env;

use ecmwf_opendata::{Client, ClientOptions, Request};

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() == 1 {
        eprintln!(
            "Usage:\n  cargo run --example cli -- retrieve <target>\n\nExample (HRES, latest, msl, +240h):\n  cargo run --example cli -- retrieve data.grib2\n\nNotes:\n- This will contact ECMWF Open Data (default source=ecmwf).\n- Downloading implies CC BY 4.0 attribution requirements (see ECMWF Open Data license)."
        );
        return;
    }

    match args.get(1).map(|s| s.as_str()) {
        Some("retrieve") => {
            let target = args
                .get(2)
                .cloned()
                .unwrap_or_else(|| "data.grib2".to_string());

            let opts = ClientOptions {
                source: "ecmwf".to_string(),
                model: "ifs".to_string(),
                resol: "0p25".to_string(),
                preserve_request_order: false,
                infer_stream_keyword: true,
                ..ClientOptions::default()
            };
            let client = Client::new(opts).expect("create client");

            let request = Request::new()
                .r#type("fc")
                .step(240)
                .param("msl")
                .target(&target);

            match client.retrieve_request(request) {
                Ok(result) => {
                    println!(
                        "Downloaded {bytes} bytes to {target}",
                        bytes = result.size_bytes
                    );
                    println!("Forecast datetime: {}", result.datetime);
                }
                Err(e) => {
                    eprintln!("retrieve failed: {e}");
                    eprintln!("Tip: try setting an explicit date/time in code, or use a replicated source (aws/google/azure) if the main portal is busy.");
                    std::process::exit(1);
                }
            }
        }

        Some("download") => {
            let target = args
                .get(2)
                .cloned()
                .unwrap_or_else(|| "data.grib2".to_string());

            let client = Client::new(ClientOptions::default()).expect("create client");
            let request = Request::new().r#type("fc").step(240).target(&target);

            match client.download_request(request) {
                Ok(result) => {
                    println!(
                        "Downloaded {bytes} bytes to {target}",
                        bytes = result.size_bytes
                    );
                    println!("Forecast datetime: {}", result.datetime);
                }
                Err(e) => {
                    eprintln!("download failed: {e}");
                    eprintln!("Tip: try setting an explicit date/time in code, or use a replicated source (aws/google/azure) if the main portal is busy.");
                    std::process::exit(1);
                }
            }
        }
        _ => {
            eprintln!("Unknown command. Use: retrieve|download");
            std::process::exit(2);
        }
    }
}
