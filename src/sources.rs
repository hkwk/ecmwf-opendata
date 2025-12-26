/// Built-in base URLs (same values as upstream ecmwf-opendata).
///
/// If `source` is already an `http(s)` URL, it is used as-is.
pub fn source_to_base_url(source: &str) -> Option<&'static str> {
    match source {
        "ecmwf" => Some("https://data.ecmwf.int/forecasts"),
        "azure" => Some("https://ai4edataeuwest.blob.core.windows.net/ecmwf"),
        "aws" => Some("https://ecmwf-forecasts.s3.eu-central-1.amazonaws.com"),
        "google" => Some("https://storage.googleapis.com/ecmwf-open-data"),
        "ecmwf-esuites" => Some("https://xdiss.ecmwf.int/ecpds/home/opendata"),
        _ => None,
    }
}

pub fn is_http_url(s: &str) -> bool {
    s.starts_with("http://") || s.starts_with("https://")
}
