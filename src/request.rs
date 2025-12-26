use std::collections::BTreeMap;

use crate::error::{Error, Result};

/// Value type for a request keyword.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RequestValue {
    Str(String),
    Int(i64),
    StrList(Vec<String>),
    IntList(Vec<i64>),
}

impl From<&str> for RequestValue {
    fn from(value: &str) -> Self {
        RequestValue::Str(value.to_string())
    }
}

impl From<String> for RequestValue {
    fn from(value: String) -> Self {
        RequestValue::Str(value)
    }
}

impl From<&String> for RequestValue {
    fn from(value: &String) -> Self {
        RequestValue::Str(value.clone())
    }
}

impl From<i64> for RequestValue {
    fn from(value: i64) -> Self {
        RequestValue::Int(value)
    }
}

impl From<i32> for RequestValue {
    fn from(value: i32) -> Self {
        RequestValue::Int(value as i64)
    }
}

impl From<u32> for RequestValue {
    fn from(value: u32) -> Self {
        RequestValue::Int(value as i64)
    }
}

impl From<usize> for RequestValue {
    fn from(value: usize) -> Self {
        RequestValue::Int(value as i64)
    }
}

impl From<Vec<String>> for RequestValue {
    fn from(value: Vec<String>) -> Self {
        RequestValue::StrList(value)
    }
}

impl From<Vec<&str>> for RequestValue {
    fn from(value: Vec<&str>) -> Self {
        RequestValue::StrList(value.into_iter().map(|s| s.to_string()).collect())
    }
}

impl<const N: usize> From<[&str; N]> for RequestValue {
    fn from(value: [&str; N]) -> Self {
        RequestValue::StrList(value.into_iter().map(|s| s.to_string()).collect())
    }
}

impl<const N: usize> From<[String; N]> for RequestValue {
    fn from(value: [String; N]) -> Self {
        RequestValue::StrList(value.into_iter().collect())
    }
}

impl From<Vec<i64>> for RequestValue {
    fn from(value: Vec<i64>) -> Self {
        RequestValue::IntList(value)
    }
}

impl From<Vec<i32>> for RequestValue {
    fn from(value: Vec<i32>) -> Self {
        RequestValue::IntList(value.into_iter().map(|x| x as i64).collect())
    }
}

impl<const N: usize> From<[i32; N]> for RequestValue {
    fn from(value: [i32; N]) -> Self {
        RequestValue::IntList(value.into_iter().map(|x| x as i64).collect())
    }
}

impl From<Vec<u32>> for RequestValue {
    fn from(value: Vec<u32>) -> Self {
        RequestValue::IntList(value.into_iter().map(|x| x as i64).collect())
    }
}

impl<const N: usize> From<[u32; N]> for RequestValue {
    fn from(value: [u32; N]) -> Self {
        RequestValue::IntList(value.into_iter().map(|x| x as i64).collect())
    }
}

impl From<Vec<usize>> for RequestValue {
    fn from(value: Vec<usize>) -> Self {
        RequestValue::IntList(value.into_iter().map(|x| x as i64).collect())
    }
}

impl<const N: usize> From<[usize; N]> for RequestValue {
    fn from(value: [usize; N]) -> Self {
        RequestValue::IntList(value.into_iter().map(|x| x as i64).collect())
    }
}

impl RequestValue {
    /// Parse a user-provided string into a best-effort [`RequestValue`].
    ///
    /// This is designed for GUI / config-file inputs where everything starts as a string.
    ///
    /// Rules (intentionally simple):
    /// - `"240"` -> `Int(240)`
    /// - `"a,b,c"` -> `StrList([..])`
    /// - `"1,10,20"` -> `IntList([..])`
    /// - `"[1, 10, 20]"` -> `IntList([..])`
    /// - Otherwise -> `Str(..)`
    ///
    /// Note: range syntaxes like `"0/to/144/by/3"` or `"0-24"` are kept as strings;
    /// expansion happens later during request normalization.
    pub fn parse_auto(s: &str) -> Self {
        let mut t = s.trim();
        if t.starts_with('[') && t.ends_with(']') && t.len() >= 2 {
            t = &t[1..t.len() - 1];
            t = t.trim();
        }

        if t.contains(',') {
            let items: Vec<&str> = t.split(',').map(|x| x.trim()).filter(|x| !x.is_empty()).collect();
            if items.is_empty() {
                return RequestValue::Str(String::new());
            }

            let mut all_int = true;
            let mut ints: Vec<i64> = Vec::with_capacity(items.len());
            let mut strs: Vec<String> = Vec::with_capacity(items.len());

            for it in &items {
                if let Ok(v) = it.parse::<i64>() {
                    ints.push(v);
                } else {
                    all_int = false;
                    strs.push(it.to_string());
                }
            }

            if all_int {
                RequestValue::IntList(ints)
            } else {
                // If mixed (e.g. steps like "0-24"), keep everything as strings.
                if strs.len() != items.len() {
                    strs = items.into_iter().map(|x| x.to_string()).collect();
                }
                RequestValue::StrList(strs)
            }
        } else if let Ok(v) = t.parse::<i64>() {
            RequestValue::Int(v)
        } else {
            RequestValue::Str(t.to_string())
        }
    }

    pub fn as_strings(&self) -> Vec<String> {
        match self {
            RequestValue::Str(s) => vec![s.clone()],
            RequestValue::Int(i) => vec![i.to_string()],
            RequestValue::StrList(xs) => xs.clone(),
            RequestValue::IntList(xs) => xs.iter().map(|x| x.to_string()).collect(),
        }
    }
}

/// MARS-like request expressed as keyword/value pairs.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Request {
    pub(crate) inner: BTreeMap<String, RequestValue>,
}

impl Request {
    pub fn new() -> Self {
        Self {
            inner: BTreeMap::new(),
        }
    }

    /// Insert a keyword/value pair (value can be a scalar or list).
    pub fn kw(mut self, key: impl Into<String>, value: impl Into<RequestValue>) -> Self {
        self.inner.insert(key.into(), value.into());
        self
    }

    /// Construct a request from an iterator of keyword/value pairs.
    pub fn from_pairs<K, V>(pairs: impl IntoIterator<Item = (K, V)>) -> Self
    where
        K: Into<String>,
        V: Into<RequestValue>,
    {
        let mut r = Self::new();
        for (k, v) in pairs {
            r = r.kw(k, v);
        }
        r
    }

    /// Construct a request from string pairs (typical for GUI/config inputs).
    /// Values are parsed with [`RequestValue::parse_auto`].
    pub fn from_str_pairs<K, V>(pairs: impl IntoIterator<Item = (K, V)>) -> Self
    where
        K: Into<String>,
        V: AsRef<str>,
    {
        let mut r = Self::new();
        for (k, v) in pairs {
            r = r.kw(k, RequestValue::parse_auto(v.as_ref()));
        }
        r
    }

    pub fn insert(mut self, key: impl Into<String>, value: RequestValue) -> Self {
        self.inner.insert(key.into(), value);
        self
    }

    pub fn set(&mut self, key: impl Into<String>, value: RequestValue) {
        self.inner.insert(key.into(), value);
    }

    // Convenience builders to mimic Python keyword arguments.
    pub fn r#type(self, v: impl Into<RequestValue>) -> Self {
        self.kw("type", v)
    }

    pub fn stream(self, v: impl Into<RequestValue>) -> Self {
        self.kw("stream", v)
    }

    pub fn date(self, v: impl Into<RequestValue>) -> Self {
        self.kw("date", v)
    }

    pub fn time(self, v: impl Into<RequestValue>) -> Self {
        self.kw("time", v)
    }

    pub fn step(self, v: impl Into<RequestValue>) -> Self {
        self.kw("step", v)
    }

    pub fn fcmonth(self, v: impl Into<RequestValue>) -> Self {
        self.kw("fcmonth", v)
    }

    pub fn param(self, v: impl Into<RequestValue>) -> Self {
        self.kw("param", v)
    }

    pub fn levtype(self, v: impl Into<RequestValue>) -> Self {
        self.kw("levtype", v)
    }

    pub fn levelist(self, v: impl Into<RequestValue>) -> Self {
        self.kw("levelist", v)
    }

    pub fn number(self, v: impl Into<RequestValue>) -> Self {
        self.kw("number", v)
    }

    pub fn model(self, v: impl Into<RequestValue>) -> Self {
        self.kw("model", v)
    }

    pub fn resol(self, v: impl Into<RequestValue>) -> Self {
        self.kw("resol", v)
    }

    pub fn target(self, v: impl Into<RequestValue>) -> Self {
        self.kw("target", v)
    }

    pub fn get(&self, key: &str) -> Option<&RequestValue> {
        self.inner.get(key)
    }

    pub fn remove(&mut self, key: &str) {
        self.inner.remove(key);
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &RequestValue)> {
        self.inner.iter()
    }

    pub fn into_inner(self) -> BTreeMap<String, RequestValue> {
        self.inner
    }

    pub(crate) fn from_inner(inner: BTreeMap<String, RequestValue>) -> Self {
        Self { inner }
    }
}

#[cfg(test)]
mod parse_tests {
    use super::{Request, RequestValue};

    #[test]
    fn parse_auto_int_and_string() {
        assert_eq!(RequestValue::parse_auto("240"), RequestValue::Int(240));
        assert_eq!(RequestValue::parse_auto("msl"), RequestValue::Str("msl".to_string()));
    }

    #[test]
    fn parse_auto_lists() {
        assert_eq!(
            RequestValue::parse_auto("1,10,20"),
            RequestValue::IntList(vec![1, 10, 20])
        );
        assert_eq!(
            RequestValue::parse_auto("[1, 10, 20]"),
            RequestValue::IntList(vec![1, 10, 20])
        );
        assert_eq!(
            RequestValue::parse_auto("2t,msl"),
            RequestValue::StrList(vec!["2t".to_string(), "msl".to_string()])
        );
        // Keep step ranges as strings.
        assert_eq!(
            RequestValue::parse_auto("0-24,12-36"),
            RequestValue::StrList(vec!["0-24".to_string(), "12-36".to_string()])
        );
    }

    #[test]
    fn from_str_pairs_builds_request() {
        let r = Request::from_str_pairs([("step", "12,24,36"), ("param", "msl")]);
        assert_eq!(r.get("step"), Some(&RequestValue::IntList(vec![12, 24, 36])));
        assert_eq!(r.get("param"), Some(&RequestValue::Str("msl".to_string())));
    }
}

/// Expand a list-like value, accepting strings like "0/to/120/by/6".
///
/// This is a minimal subset of the upstream Python expansion rules, sufficient
/// for `step`, `time`, and `date`.
pub fn expand_numeric_syntax(s: &str) -> Result<Vec<String>> {
    let tokens: Vec<&str> = s.split('/').filter(|t| !t.is_empty()).collect();
    if tokens.len() == 3 && tokens[1].eq_ignore_ascii_case("to") {
        // a/to/b
        let start: i64 = tokens[0].parse().map_err(|_| {
            Error::InvalidRequest(format!("cannot parse range start {tokens:?}"))
        })?;
        let end: i64 = tokens[2].parse().map_err(|_| {
            Error::InvalidRequest(format!("cannot parse range end {tokens:?}"))
        })?;
        if end < start {
            return Err(Error::InvalidRequest(format!(
                "range end {end} < start {start}"
            )));
        }
        return Ok((start..=end).map(|x| x.to_string()).collect());
    }

    if tokens.len() == 5
        && tokens[1].eq_ignore_ascii_case("to")
        && tokens[3].eq_ignore_ascii_case("by")
    {
        // a/to/b/by/step
        let start: i64 = tokens[0].parse().map_err(|_| {
            Error::InvalidRequest(format!("cannot parse range start {tokens:?}"))
        })?;
        let end: i64 = tokens[2].parse().map_err(|_| {
            Error::InvalidRequest(format!("cannot parse range end {tokens:?}"))
        })?;
        let by: i64 = tokens[4].parse().map_err(|_| {
            Error::InvalidRequest(format!("cannot parse range step {tokens:?}"))
        })?;
        if by <= 0 {
            return Err(Error::InvalidRequest(format!("range step must be >0, got {by}")));
        }
        if end < start {
            return Err(Error::InvalidRequest(format!(
                "range end {end} < start {start}"
            )));
        }

        let mut out = Vec::new();
        let mut cur = start;
        while cur <= end {
            out.push(cur.to_string());
            cur += by;
        }
        return Ok(out);
    }

    Ok(vec![s.to_string()])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_single_value() {
        assert_eq!(expand_numeric_syntax("0").unwrap(), vec!["0"]);
    }

    #[test]
    fn expands_to_range_inclusive() {
        assert_eq!(
            expand_numeric_syntax("0/to/3").unwrap(),
            vec!["0", "1", "2", "3"]
        );
    }

    #[test]
    fn expands_to_range_with_by() {
        assert_eq!(
            expand_numeric_syntax("0/to/12/by/6").unwrap(),
            vec!["0", "6", "12"]
        );
    }
}
