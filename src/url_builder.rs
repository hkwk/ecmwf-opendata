use chrono::{DateTime, Utc};

use crate::date::end_step;

pub const HOURLY_PATTERN: &str = "{url}/{yyyymmdd}/{H}z/{model}/{resol}/{stream}/{yyyymmddHHMMSS}-{step}h-{stream}-{type}.{ext}";
pub const MONTHLY_PATTERN: &str = "{url}/{yyyymmdd}/{H}z/{model}/{resol}/{stream}/{yyyymmddHHMMSS}-{fcmonth}m-{stream}-{type}.{ext}";

pub fn extension_for_type(typ: &str) -> &'static str {
    if typ == "tf" {
        "bufr"
    } else {
        "grib2"
    }
}

pub fn user_to_url_value(model: &str, key: &str, value: &str, all_url_type_values: &[String]) -> String {
    // Mirrors upstream mapping.
    // type mapping affects file naming.
    if key == "type" {
        if model == "aifs-ens" {
            // aifs-ens does not currently use "ef" in filenames.
            if value == "pf" || value == "cf" {
                return value.to_string();
            }
        }
        return match value {
            "cf" | "pf" => "ef".to_string(),
            "em" | "es" => "ep".to_string(),
            "fcmean" => "fc".to_string(),
            _ => value.to_string(),
        };
    }

    if key == "stream" {
        return match value {
            "mmsa" => "mmsf".to_string(),
            _ => value.to_string(),
        };
    }

    if key == "step" {
        // For probabilities, the URL contains either 240 or 360.
        if all_url_type_values.len() == 1 && all_url_type_values[0] == "ep" {
            if let Some(e) = end_step(value) {
                return if e <= 240 { "240".to_string() } else { "360".to_string() };
            }
        }
    }

    value.to_string()
}

pub fn patch_stream(infer_stream_keyword: bool, model: &str, stream: &str, hour_2d: &str, typ: &str) -> String {
    if !infer_stream_keyword || model == "aifs-single" {
        return stream.to_string();
    }

    // First patch based on hour.
    let mut s = match (stream, hour_2d) {
        ("oper", "06") | ("oper", "18") => "scda",
        ("wave", "06") | ("wave", "18") => "scwv",
        _ => stream,
    }
    .to_string();

    // Then patch based on type.
    s = match (s.as_str(), typ) {
        ("oper", "ef") => "enfo".to_string(),
        ("wave", "ef") => "waef".to_string(),
        ("oper", "ep") => "enfo".to_string(),
        ("wave", "ep") => "waef".to_string(),
        ("scda", "ef") => "enfo".to_string(),
        ("scwv", "ef") => "waef".to_string(),
        ("scda", "ep") => "enfo".to_string(),
        ("scwv", "ep") => "waef".to_string(),
        _ => s,
    };

    s
}

pub fn format_url(
    pattern: &str,
    base_url: &str,
    date: DateTime<Utc>,
    model: &str,
    resol: &str,
    stream: &str,
    typ: &str,
    step: Option<&str>,
    fcmonth: Option<&str>,
) -> String {
    let yyyymmdd = date.format("%Y%m%d").to_string();
    let hh = date.format("%H").to_string();
    let yyyymmdd_hhmmss = date.format("%Y%m%d%H%M%S").to_string();
    let ext = extension_for_type(typ);

    let mut url = pattern
        .replace("{url}", base_url)
        .replace("{yyyymmdd}", &yyyymmdd)
        .replace("{H}", &hh)
        .replace("{model}", model)
        .replace("{resol}", resol)
        .replace("{stream}", stream)
        .replace("{type}", typ)
        .replace("{yyyymmddHHMMSS}", &yyyymmdd_hhmmss)
        .replace("{ext}", ext);

    if let Some(step) = step {
        url = url.replace("{step}", step);
    }
    if let Some(fcmonth) = fcmonth {
        url = url.replace("{fcmonth}", fcmonth);
    }

    url
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patch_stream_infers_scda_scwv() {
        assert_eq!(patch_stream(true, "ifs", "oper", "00", "fc"), "oper");
        assert_eq!(patch_stream(true, "ifs", "oper", "06", "fc"), "scda");
        assert_eq!(patch_stream(true, "ifs", "wave", "18", "fc"), "scwv");
    }

    #[test]
    fn patch_stream_infers_ens_streams_from_type() {
        assert_eq!(patch_stream(true, "ifs", "oper", "00", "ef"), "enfo");
        assert_eq!(patch_stream(true, "ifs", "wave", "00", "ep"), "waef");
    }

    #[test]
    fn user_to_url_value_type_mapping() {
        assert_eq!(user_to_url_value("ifs", "type", "cf", &[]), "ef");
        assert_eq!(user_to_url_value("ifs", "type", "pf", &[]), "ef");
        assert_eq!(user_to_url_value("ifs", "type", "em", &[]), "ep");

        assert_eq!(user_to_url_value("aifs-ens", "type", "cf", &[]), "cf");
        assert_eq!(user_to_url_value("aifs-ens", "type", "pf", &[]), "pf");
    }
}
