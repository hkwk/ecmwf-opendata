use std::collections::{BTreeMap, BTreeSet};
use std::fs::OpenOptions;
use std::io::{Read, Write};

use chrono::{DateTime, Datelike, Duration, TimeZone, Timelike, Utc};
use reqwest::blocking::Client as HttpClient;
use reqwest::header::{HeaderMap, HeaderValue, RANGE, USER_AGENT};

use crate::date::{canonical_time_to_hour, expand_date_value, expand_time_value, full_datetime_from_date_time};
use crate::error::{Error, Result as EResult};
use crate::request::{expand_numeric_syntax, Request, RequestValue};
use crate::sources::{is_http_url, source_to_base_url};
use crate::url_builder::{format_url, patch_stream, user_to_url_value, HOURLY_PATTERN, MONTHLY_PATTERN};

const URL_COMPONENTS: [&str; 8] = [
    "date", "time", "model", "resol", "stream", "type", "step", "fcmonth",
];

const INDEX_COMPONENTS: [&str; 6] = ["param", "type", "step", "fcmonth", "number", "levelist"];

#[derive(Debug, Clone)]
pub struct ClientOptions {
    pub source: String,
    pub model: String,
    pub resol: String,
    pub beta: bool,
    pub preserve_request_order: bool,
    pub infer_stream_keyword: bool,
    pub verify_tls: bool,
    pub use_sas_token: Option<bool>,
    pub sas_known_key: String,
    pub sas_custom_url: Option<String>,
}

impl Default for ClientOptions {
    fn default() -> Self {
        Self {
            source: "ecmwf".to_string(),
            model: "ifs".to_string(),
            resol: "0p25".to_string(),
            beta: false,
            preserve_request_order: false,
            infer_stream_keyword: true,
            verify_tls: true,
            use_sas_token: None,
            sas_known_key: "ecmwf".to_string(),
            sas_custom_url: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Result {
    pub urls: Vec<String>,
    pub target: String,
    pub datetime: DateTime<Utc>,
    pub for_urls: BTreeMap<String, Vec<String>>,
    pub for_index: BTreeMap<String, Vec<String>>,
    pub size_bytes: u64,
}

#[derive(Debug, Clone)]
pub struct Client {
    opts: ClientOptions,
    base_url: String,
    http: HttpClient,
    sas_token: Option<String>,
}

impl Client {
    pub fn new(opts: ClientOptions) -> EResult<Self> {
        let base_url = if is_http_url(&opts.source) {
            opts.source.clone()
        } else {
            source_to_base_url(&opts.source)
                .ok_or_else(|| Error::InvalidRequest(format!("unknown source: {}", opts.source)))?
                .to_string()
        };

        let mut headers = HeaderMap::new();
        headers.insert(
            USER_AGENT,
            HeaderValue::from_static("ecmwf-opendata-rs/0.1"),
        );

        let mut builder = HttpClient::builder().default_headers(headers);
        if !opts.verify_tls {
            builder = builder.danger_accept_invalid_certs(true);
        }
        let http = builder.build()?;

        let use_sas = opts
            .use_sas_token
            .unwrap_or_else(|| opts.source == "azure");

        let mut client = Self {
            base_url,
            http,
            opts,
            sas_token: None,
        };

        if use_sas {
            let token = client.get_azure_sas_token()?;
            client.sas_token = Some(token);
        }

        Ok(client)
    }

    pub fn retrieve(&self, request: Request, target: impl Into<String>) -> EResult<Result> {
        let target = target.into();
        let res = self.get_urls(Some(&request), true, Some(&target))?;
        self.download_result(&res, true)
    }

    /// Python-like convenience: `retrieve(request)` where `target` may be inside the request.
    /// If no target is provided, defaults to `data.grib2`.
    pub fn retrieve_request(&self, request: Request) -> EResult<Result> {
        let res = self.get_urls(Some(&request), true, None)?;
        self.download_result(&res, true)
    }

    /// Python-kwargs-like convenience: build a request from pairs and retrieve it.
    ///
    /// Example:
    /// `client.retrieve_pairs([("step", 240), ("type", "fc"), ("param", "msl")])?;`
    pub fn retrieve_pairs<K>(
        &self,
        pairs: impl IntoIterator<Item = (K, RequestValue)>,
    ) -> EResult<Result>
    where
        K: Into<String>,
    {
        self.retrieve_request(Request::from_pairs(pairs))
    }

    pub fn download(&self, request: Request, target: impl Into<String>) -> EResult<Result> {
        let target = target.into();
        let res = self.get_urls(Some(&request), false, Some(&target))?;
        self.download_result(&res, false)
    }

    /// Python-like convenience: `download(request)` where `target` may be inside the request.
    /// If no target is provided, defaults to `data.grib2`.
    pub fn download_request(&self, request: Request) -> EResult<Result> {
        let res = self.get_urls(Some(&request), false, None)?;
        self.download_result(&res, false)
    }

    pub fn latest(&self, request: Request) -> EResult<DateTime<Utc>> {
        self.latest_inner(&request)
    }

    /// Convenience constructor similar to Python's `Client()` defaults.
    pub fn default_client() -> EResult<Self> {
        Self::new(ClientOptions::default())
    }

    fn latest_inner(&self, request: &Request) -> EResult<DateTime<Utc>> {
        let mut params = request.clone().into_inner();

        let now = Utc::now();

        // If time not in request: probe the most recent 6-hour cycle and step back by 6 hours.
        // If time is in request: keep that hour and step back by 1 day.
        let has_time = params.contains_key("time");
        let delta = if has_time { Duration::days(1) } else { Duration::hours(6) };

        let time_hour = if let Some(tv) = params.get("time") {
            let t = tv.as_strings().get(0).cloned().unwrap_or_else(|| "18".into());
            canonical_time_to_hour(&t)?
        } else {
            18
        };

        let mut candidate = if has_time {
            // Start at today with that hour, but never in the future.
            let start_date = now.date_naive();
            let mut dt = Utc
                .with_ymd_and_hms(
                    start_date.year(),
                    start_date.month(),
                    start_date.day(),
                    time_hour,
                    0,
                    0,
                )
                .single()
                .ok_or_else(|| Error::InvalidRequest("invalid start datetime".into()))?;
            if dt > now {
                dt = dt - Duration::days(1);
            }
            dt
        } else {
            // Round down to the nearest 6-hour cycle: 00/06/12/18.
            let hour = (now.hour() / 6) * 6;
            Utc.with_ymd_and_hms(now.year(), now.month(), now.day(), hour, 0, 0)
                .single()
                .ok_or_else(|| Error::InvalidRequest("invalid start datetime".into()))?
        };

        // Search back up to ~5 days.
        let stop = candidate - Duration::days(5);

        loop {
            if candidate <= stop {
                break;
            }

            params.insert(
                "date".to_string(),
                RequestValue::Str(candidate.format("%Y%m%d").to_string()),
            );
            let probe_hour: u32 = if has_time {
                time_hour
            } else {
                candidate.hour()
            };
            params.insert("time".to_string(), RequestValue::Int(probe_hour as i64));

            let tmp_req = Request::from_inner(params.clone());
            let res = self.get_urls(Some(&tmp_req), false, None)?;

            let mut ok = !res.urls.is_empty();
            for u in &res.urls {
                let url = self.apply_sas_to_url(u);
                if !self.probe_exists(&url)? {
                    ok = false;
                    break;
                }
            }
            if ok {
                return Ok(candidate);
            }

            candidate = candidate - delta;
        }

        Err(Error::CannotEstablishLatest)
    }

    /// Probe a URL for existence.
    ///
    /// Upstream Python uses HTTP HEAD. Some endpoints may block HEAD or respond
    /// with non-200 even though GET works; in that case we fall back to a tiny
    /// ranged GET.
    fn probe_exists(&self, url: &str) -> EResult<bool> {
        // Try HEAD first (cheap when supported).
        match self.http.head(url).send() {
            Ok(resp) => {
                if resp.status() == 200 {
                    return Ok(true);
                }

                // If HEAD is not usable, fall back to a ranged GET.
                if matches!(
                    resp.status().as_u16(),
                    403 | 404 | 405 | 409 | 429 | 500 | 501 | 502 | 503
                ) {
                    // continue to GET probe
                } else {
                    return Ok(false);
                }
            }
            Err(_) => {
                // Fall back to GET probe.
            }
        }

        // GET with a single byte range; accept 206 (partial) or 200.
        let resp = self
            .http
            .get(url)
            .header(RANGE, "bytes=0-0")
            .send()?;

        Ok(matches!(resp.status().as_u16(), 200 | 206))
    }

    fn get_urls(
        &self,
        request: Option<&Request>,
        use_index: bool,
        target: Option<&str>,
    ) -> EResult<Result> {
        let mut params = match request {
            Some(r) => r.clone().into_inner(),
            None => BTreeMap::new(),
        };

        // defaults
        let model = params
            .get("model")
            .map(|v| v.as_strings().get(0).cloned().unwrap_or_else(|| self.opts.model.clone()))
            .unwrap_or_else(|| self.opts.model.clone());

        if model == "aifs-ens" && !params.contains_key("stream") {
            params.insert("stream".to_string(), RequestValue::Str("enfo".to_string()));
        }

        params.entry("model".to_string()).or_insert(RequestValue::Str(model.clone()));
        params
            .entry("resol".to_string())
            .or_insert(RequestValue::Str(self.opts.resol.clone()));

        params.entry("type".to_string()).or_insert(RequestValue::Str("fc".to_string()));
        params
            .entry("stream".to_string())
            .or_insert(RequestValue::Str("oper".to_string()));
        params.entry("step".to_string()).or_insert(RequestValue::Int(0));

        // If date missing, resolve latest.
        if !params.contains_key("date") {
            let tmp_req = Request::from_inner(params.clone());
            let latest = self.latest_inner(&tmp_req)?;
            params.insert(
                "date".to_string(),
                RequestValue::Str(latest.format("%Y%m%d").to_string()),
            );
            // Keep request's time if present; else use latest hour.
            if !params.contains_key("time") {
                params.insert("time".to_string(), RequestValue::Int(latest.hour() as i64));
            }
        }

        // Normalize / expand into for_urls and for_index
        let now = Utc::now();

        let mut for_urls: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let mut for_index: BTreeMap<String, Vec<String>> = BTreeMap::new();

        // Build for_urls types first to allow step mapping for probabilities.
        let typ_values_user: Vec<String> = params
            .get("type")
            .map(|v| v.as_strings())
            .unwrap_or_else(|| vec!["fc".to_string()]);

        let mut for_urls_type: Vec<String> = Vec::new();
        for tv in typ_values_user {
            for_urls_type.push(user_to_url_value(&model, "type", &tv, &[]));
        }
        if for_urls_type.is_empty() {
            for_urls_type.push("fc".to_string());
        }
        for_urls.insert("type".to_string(), unique_preserve(for_urls_type));

        // Process each param
        for (k, v) in params.iter() {
            let mut values = v.as_strings();

            // allow slash-separated lists
            if values.len() == 1 && values[0].contains('/') {
                values = values[0]
                    .split('/')
                    .filter(|t| !t.is_empty())
                    .map(|t| t.to_string())
                    .collect();
            }

            let expanded: Vec<String> = match k.as_str() {
                "date" => {
                    let mut out = Vec::new();
                    for x in values {
                        out.extend(expand_date_value(&x, now)?);
                    }
                    out
                }
                "time" => {
                    let mut out = Vec::new();
                    for x in values {
                        out.extend(expand_time_value(&x)?);
                    }
                    out
                }
                "step" | "fcmonth" | "number" | "levelist" => {
                    let mut out = Vec::new();
                    for x in values {
                        out.extend(expand_numeric_syntax(&x)?);
                    }
                    out
                }
                _ => values,
            };

            if URL_COMPONENTS.contains(&k.as_str()) {
                let mut mapped = Vec::new();
                for x in &expanded {
                    let url_t = for_urls.get("type").cloned().unwrap_or_default();
                    mapped.push(user_to_url_value(&model, k, x, &url_t));
                }
                for_urls
                    .entry(k.clone())
                    .or_default()
                    .extend(mapped);
            }

            if INDEX_COMPONENTS.contains(&k.as_str()) {
                // user_to_index: type=ef expands to cf/pf for index selection.
                let mut mapped = Vec::new();
                if k == "type" {
                    for x in &expanded {
                        if x == "ef" {
                            mapped.push("cf".to_string());
                            mapped.push("pf".to_string());
                        } else {
                            mapped.push(x.clone());
                        }
                    }
                } else {
                    mapped = expanded.clone();
                }
                for_index.entry(k.clone()).or_default().extend(mapped);
            }
        }

        // Canonicalize time: store hour string (00/06/12/18)
        if let Some(times) = for_urls.get_mut("time") {
            let mut out = Vec::new();
            for t in times.drain(..) {
                let hour = canonical_time_to_hour(&t)?;
                out.push(format!("{hour:02}"));
            }
            *times = unique_preserve(out);
        }

        // Infer/patch stream in URL building; we keep stream values but will patch later per product.
        for (k, vals) in for_urls.iter_mut() {
            *vals = unique_preserve(std::mem::take(vals));
            if k == "stream" || k == "type" {
                vals.iter_mut().for_each(|s| s.make_ascii_lowercase());
            }
        }
        for (k, vals) in for_index.iter_mut() {
            *vals = unique_preserve(std::mem::take(vals));
            if k == "stream" || k == "type" {
                vals.iter_mut().for_each(|s| s.make_ascii_lowercase());
            }
        }

        // If tf (tropical cyclone tracks), do not use index selection.
        let user_type = params
            .get("type")
            .map(|v| v.as_strings().get(0).cloned().unwrap_or_else(|| "fc".into()))
            .unwrap_or_else(|| "fc".into());
        if user_type == "tf" {
            for_index.clear();
        }

        // If time missing (possible if date contains time), default time based on date.
        if !for_urls.contains_key("time") {
            for_urls.insert("time".to_string(), vec!["18".to_string()]);
        }

        // Now expand into concrete URLs
        let mut urls = Vec::new();
        let mut dates = BTreeSet::new();

        let date_vals = for_urls
            .get("date")
            .cloned()
            .ok_or_else(|| Error::InvalidRequest("date missing after normalization".into()))?;
        let time_vals = for_urls
            .get("time")
            .cloned()
            .ok_or_else(|| Error::InvalidRequest("time missing after normalization".into()))?;

        let model_vals = for_urls.get("model").cloned().unwrap_or_else(|| vec![model.clone()]);
        let resol_vals = for_urls
            .get("resol")
            .cloned()
            .unwrap_or_else(|| vec![self.opts.resol.clone()]);
        let stream_vals = for_urls
            .get("stream")
            .cloned()
            .unwrap_or_else(|| vec!["oper".to_string()]);
        let type_vals = for_urls
            .get("type")
            .cloned()
            .unwrap_or_else(|| vec!["fc".to_string()]);
        let step_vals = for_urls.get("step").cloned().unwrap_or_else(|| vec!["0".to_string()]);
        let fcmonth_vals = for_urls
            .get("fcmonth")
            .cloned()
            .unwrap_or_else(|| vec!["1".to_string()]);

        for d in &date_vals {
            for t in &time_vals {
                let dt = full_datetime_from_date_time(d, t.parse::<u32>().map_err(|_| {
                    Error::InvalidRequest(format!("invalid canonical time hour: {t}"))
                })?)?;
                dates.insert(dt);

                for m in &model_vals {
                    for r in &resol_vals {
                        for s in &stream_vals {
                            for ty in &type_vals {
                                // patch stream based on time and type
                                let hour_2d = dt.format("%H").to_string();
                                let patched_stream = patch_stream(
                                    self.opts.infer_stream_keyword,
                                    m,
                                    s,
                                    &hour_2d,
                                    ty,
                                );

                                let is_monthly = s == "mmsa";
                                let pattern = if is_monthly {
                                    MONTHLY_PATTERN
                                } else {
                                    HOURLY_PATTERN
                                };

                                // beta tweaks
                                let mut resol = r.clone();
                                if self.opts.beta {
                                    resol = format!("{resol}/experimental");
                                }

                                if is_monthly {
                                    for fcmonth in &fcmonth_vals {
                                        let u = format_url(
                                            pattern,
                                            &self.base_url,
                                            dt,
                                            m,
                                            &resol,
                                            &patched_stream,
                                            ty,
                                            None,
                                            Some(fcmonth),
                                        );
                                        urls.push(self.fix_0p4_beta(u));
                                    }
                                } else {
                                    for step in &step_vals {
                                        let u = format_url(
                                            pattern,
                                            &self.base_url,
                                            dt,
                                            m,
                                            &resol,
                                            &patched_stream,
                                            ty,
                                            Some(step),
                                            None,
                                        );
                                        urls.push(self.fix_0p4_beta(u));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        urls = unique_preserve(urls);

        let dt = *dates
            .iter()
            .next()
            .ok_or_else(|| Error::InvalidRequest("no datetime".into()))?;

        let target_path = target
            .map(|s| s.to_string())
            .or_else(|| params.get("target").map(|v| v.as_strings().get(0).cloned()).flatten())
            .unwrap_or_else(|| "data.grib2".to_string());

        let mut res = Result {
            urls,
            target: target_path,
            datetime: dt,
            for_urls,
            for_index,
            size_bytes: 0,
        };

        if use_index && !res.for_index.is_empty() {
            res.urls = self.expand_urls_to_ranges(&res.urls, &res.for_index)?;
        }

        Ok(res)
    }

    fn fix_0p4_beta(&self, url: String) -> String {
        if self.opts.resol == "0p4-beta" {
            url.replace("/ifs/", "/")
        } else {
            url
        }
    }

    fn get_azure_sas_token(&self) -> EResult<String> {
        let known = match self.opts.sas_known_key.as_str() {
            "ecmwf" => Some("https://planetarycomputer.microsoft.com/api/sas/v1/token/ai4edataeuwest/ecmwf"),
            _ => None,
        };

        let url = if let Some(u) = known {
            u.to_string()
        } else if let Some(custom) = &self.opts.sas_custom_url {
            custom.clone()
        } else {
            return Err(Error::InvalidRequest(
                "no known sas token url and no custom provided".into(),
            ));
        };

        let v: serde_json::Value = self.http.get(url).send()?.error_for_status()?.json()?;
        let token = v
            .get("token")
            .and_then(|x| x.as_str())
            .ok_or_else(|| Error::InvalidRequest("invalid sas token response".into()))?;
        Ok(token.to_string())
    }

    fn apply_sas_to_url(&self, url: &str) -> String {
        let Some(token) = &self.sas_token else {
            return url.to_string();
        };
        if url.contains("sig=") {
            return url.to_string();
        }
        if url.contains('?') {
            format!("{url}&{token}")
        } else {
            format!("{url}?{token}")
        }
    }

    /// Expand each data URL to (url, ranges) by reading its `.index`.
    ///
    /// This returns a list of synthetic URLs with embedded range data encoded as
    /// `url|start-end;start-end;...`.
    /// The actual download uses these to issue HTTP Range requests.
    fn expand_urls_to_ranges(
        &self,
        urls: &[String],
        for_index: &BTreeMap<String, Vec<String>>,
    ) -> EResult<Vec<String>> {
        // Keep index keyword order consistent with upstream.
        let ordered_keys: Vec<&str> = INDEX_COMPONENTS
            .iter()
            .copied()
            .filter(|k| for_index.contains_key(*k))
            .collect();

        let mut out = Vec::new();
        for url in urls {
            let base = url.rsplit_once('.').map(|(b, _)| b).unwrap_or(url);
            let index_url = format!("{base}.index");
            let index_url = self.apply_sas_to_url(&index_url);

            let resp = self.http.get(index_url).send()?.error_for_status()?;
            let mut body = String::new();
            let mut reader = resp;
            reader.read_to_string(&mut body)?;

            if ordered_keys.is_empty() {
                // No index keywords, nothing to do.
                out.push(url.clone());
                continue;
            }

            if self.opts.preserve_request_order {
                // (sort_key, (offset,length)) where sort_key is a lexicographic tuple
                // capturing requested keyword/value order.
                let mut parts: Vec<(Vec<(usize, usize)>, (u64, u64))> = Vec::new();

                for line in body.lines() {
                    if line.trim().is_empty() {
                        continue;
                    }
                    let v: serde_json::Value = serde_json::from_str(line)?;
                    let offset = v
                        .get("_offset")
                        .and_then(|x| x.as_u64())
                        .ok_or_else(|| Error::InvalidRequest("index missing _offset".into()))?;
                    let length = v
                        .get("_length")
                        .and_then(|x| x.as_u64())
                        .ok_or_else(|| Error::InvalidRequest("index missing _length".into()))?;

                    let mut key: Vec<(usize, usize)> = Vec::with_capacity(ordered_keys.len());

                    let mut ok = true;
                    for (i, k) in ordered_keys.iter().enumerate() {
                        let Some(val) = v.get(*k).and_then(|x| x.as_str()) else {
                            ok = false;
                            break;
                        };
                        let allowed = for_index
                            .get(*k)
                            .ok_or_else(|| Error::InvalidRequest("internal for_index missing key".into()))?;
                        let Some(j) = allowed.iter().position(|a| a == val) else {
                            ok = false;
                            break;
                        };
                        key.push((i, j));
                    }

                    if ok {
                        parts.push((key, (offset, length)));
                    }
                }

                if parts.is_empty() {
                    continue;
                }

                parts.sort_by(|a, b| a.0.cmp(&b.0));

                let ranges: Vec<(u64, u64)> = parts.into_iter().map(|(_, r)| r).collect();
                let merged = merge_ranges(ranges);

                let mut enc = String::new();
                for (i, (start, end)) in merged.iter().enumerate() {
                    if i > 0 {
                        enc.push(';');
                    }
                    enc.push_str(&format!("{start}-{end}"));
                }

                out.push(format!("{url}|{enc}"));
            } else {
                // Fast path: sort by file offset (minimize HTTP requests).
                let mut matches: Vec<(u64, u64)> = Vec::new();

                for line in body.lines() {
                    if line.trim().is_empty() {
                        continue;
                    }
                    let v: serde_json::Value = serde_json::from_str(line)?;
                    let offset = v
                        .get("_offset")
                        .and_then(|x| x.as_u64())
                        .ok_or_else(|| Error::InvalidRequest("index missing _offset".into()))?;
                    let length = v
                        .get("_length")
                        .and_then(|x| x.as_u64())
                        .ok_or_else(|| Error::InvalidRequest("index missing _length".into()))?;

                    let mut ok = true;
                    for k in &ordered_keys {
                        let Some(val) = v.get(*k).and_then(|x| x.as_str()) else {
                            ok = false;
                            break;
                        };
                        let allowed = for_index
                            .get(*k)
                            .ok_or_else(|| Error::InvalidRequest("internal for_index missing key".into()))?;
                        if !allowed.iter().any(|a| a == val) {
                            ok = false;
                            break;
                        }
                    }

                    if ok {
                        matches.push((offset, length));
                    }
                }

                if matches.is_empty() {
                    continue;
                }

                matches.sort_by_key(|(o, _)| *o);
                let merged = merge_ranges(matches);

                let mut enc = String::new();
                for (i, (start, end)) in merged.iter().enumerate() {
                    if i > 0 {
                        enc.push(';');
                    }
                    enc.push_str(&format!("{start}-{end}"));
                }

                out.push(format!("{url}|{enc}"));
            }
        }

        if out.is_empty() {
            return Err(Error::NoMatchingIndex);
        }

        Ok(out)
    }

    fn download_result(&self, res: &Result, is_partial: bool) -> EResult<Result> {
        let mut total: u64 = 0;
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&res.target)?;

        for u in &res.urls {
            if is_partial {
                let (url, ranges) = split_url_ranges(u)?;
                for (start, end) in ranges {
                    let url = self.apply_sas_to_url(url);
                    let range_header = format!("bytes={start}-{end}");
                    let mut resp = self
                        .http
                        .get(url)
                        .header(RANGE, range_header)
                        .send()?
                        .error_for_status()?;
                    let mut buf = Vec::new();
                    resp.copy_to(&mut buf)?;
                    file.write_all(&buf)?;
                    total += buf.len() as u64;
                }
            } else {
                let url = self.apply_sas_to_url(u);
                let mut resp = self.http.get(url).send()?.error_for_status()?;
                let mut buf = Vec::new();
                resp.copy_to(&mut buf)?;
                file.write_all(&buf)?;
                total += buf.len() as u64;
            }
        }

        let mut out = res.clone();
        out.size_bytes = total;
        Ok(out)
    }
}

fn unique_preserve(xs: Vec<String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for x in xs {
        if seen.insert(x.clone()) {
            out.push(x);
        }
    }
    out
}

fn merge_ranges(mut matches: Vec<(u64, u64)>) -> Vec<(u64, u64)> {
    // input is (offset, length) -> convert to inclusive (start,end)
    if matches.is_empty() {
        return Vec::new();
    }
    if matches.len() == 1 {
        let (o, l) = matches[0];
        return vec![(o, o + l - 1)];
    }

    // Ensure sorted by offset.
    matches.sort_by_key(|(o, _)| *o);

    let mut out: Vec<(u64, u64)> = Vec::new();
    for (o, l) in matches {
        let start = o;
        let end = o + l - 1;
        if let Some(last) = out.last_mut() {
            if start <= last.1 + 1 {
                last.1 = last.1.max(end);
                continue;
            }
        }
        out.push((start, end));
    }
    out
}

fn split_url_ranges(s: &str) -> EResult<(&str, Vec<(u64, u64)>)> {
    let Some((url, enc)) = s.split_once('|') else {
        return Err(Error::InvalidRequest("expected ranged url encoding".into()));
    };

    let mut ranges = Vec::new();
    for part in enc.split(';').filter(|p| !p.is_empty()) {
        let Some((a, b)) = part.split_once('-') else {
            return Err(Error::InvalidRequest(format!("bad range: {part}")));
        };
        let start: u64 = a.parse().map_err(|_| Error::InvalidRequest(format!("bad range: {part}")))?;
        let end: u64 = b.parse().map_err(|_| Error::InvalidRequest(format!("bad range: {part}")))?;
        if end < start {
            return Err(Error::InvalidRequest(format!("bad range: {part}")));
        }
        ranges.push((start, end));
    }

    Ok((url, ranges))
}
