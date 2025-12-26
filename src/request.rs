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

impl From<Vec<u32>> for RequestValue {
    fn from(value: Vec<u32>) -> Self {
        RequestValue::IntList(value.into_iter().map(|x| x as i64).collect())
    }
}

impl RequestValue {
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
