//! The one data source: `raveld`, the ravel daemon on 127.0.0.1:7881, which
//! holds every soul's ravel store and answers for each by its short id.
//!
//! **Read-only by construction.** raveld also exposes `/sync`, `/import`,
//! `/memory/run` and `/shutdown`. `memory/run` spends real money on the
//! memory index, and `shutdown` stops the daemon for every other client on
//! the machine. A viewer has no business near either, so this client has no
//! method that reaches them, and the browser can only reach what the routes
//! in `main.rs` name.

use serde_json::Value;

pub const DAEMON_PORT: u16 = 7881;

pub const MEMORY_GRAPH: &str = "https://repolex.ai/ravel/NamedGraph/memory-v1";
pub const RAVEL: &str = "https://repolex.ai/ontology/ravel/";

#[derive(Clone)]
pub struct Raveld {
    http: reqwest::Client,
    base: String,
}

impl Raveld {
    pub fn new(port: u16) -> Self {
        Raveld {
            http: reqwest::Client::new(),
            base: format!("http://127.0.0.1:{port}"),
        }
    }

    pub fn port_url(&self) -> &str {
        &self.base
    }

    /// GET a path and return its JSON, or a sentence saying why not.
    pub async fn get(&self, path: &str) -> Result<Value, String> {
        let url = format!("{}{path}", self.base);
        let r = self.http.get(&url).send().await.map_err(|e| self.down(e))?;
        let status = r.status();
        let body = r.text().await.map_err(|e| e.to_string())?;
        if !status.is_success() {
            return Err(format!("raveld answered {status} for {path}: {body}"));
        }
        serde_json::from_str(&body).map_err(|e| format!("raveld sent something that is not JSON for {path}: {e}"))
    }

    /// Run a SPARQL query against one soul and return its rows, each a map
    /// from variable name to plain string value.
    pub async fn select(&self, soul: &str, query: &str) -> Result<Vec<serde_json::Map<String, Value>>, String> {
        let url = format!("{}/souls/{soul}/query", self.base);
        let r = self
            .http
            .post(&url)
            .json(&serde_json::json!({ "query": query }))
            .send()
            .await
            .map_err(|e| self.down(e))?;
        let status = r.status();
        let body = r.text().await.map_err(|e| e.to_string())?;
        if !status.is_success() {
            return Err(format!("raveld refused a query for {soul} ({status}): {body}"));
        }
        let v: Value = serde_json::from_str(&body).map_err(|e| e.to_string())?;
        // A body without a bindings array is an error, not an empty answer.
        // Zero rows is the reassuring reading, and it must never be the
        // fallback for a reply we did not understand.
        let rows = v
            .pointer("/results/bindings")
            .and_then(Value::as_array)
            .ok_or_else(|| format!("raveld's reply for {soul} has no results.bindings"))?;
        Ok(rows
            .iter()
            .map(|row| {
                row.as_object()
                    .map(|o| {
                        o.iter()
                            .map(|(k, cell)| (k.clone(), cell.get("value").cloned().unwrap_or(Value::Null)))
                            .collect()
                    })
                    .unwrap_or_default()
            })
            .collect())
    }

    fn down(&self, e: reqwest::Error) -> String {
        if e.is_connect() {
            format!("raveld is not answering on {} — start it with `ravel daemon`", self.base)
        } else {
            e.to_string()
        }
    }
}

/// A soul id or a memory node id, as it may appear inside a query or a path.
///
/// raveld's ids are short hex (`e3d71e`), memory ids (`m1bcb0eb0d5defa5`)
/// and window ids (`w3-3405`). Anything else is refused rather than escaped:
/// these values are spliced into SPARQL and URLs, and an id that needs
/// escaping is not an id.
pub fn valid_id(s: &str) -> bool {
    !s.is_empty() && s.len() <= 64 && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// Quote free text as a SPARQL string literal.
pub fn sparql_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {}
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_that_would_need_escaping_are_refused() {
        assert!(valid_id("e3d71e"));
        assert!(valid_id("w3-3405"));
        assert!(valid_id("m1bcb0eb0d5defa5"));
        assert!(!valid_id(""));
        assert!(!valid_id("e3> } DROP ALL {"));
        assert!(!valid_id("../shutdown"));
    }

    #[test]
    fn a_quote_cannot_close_the_literal() {
        assert_eq!(sparql_string(r#"a"b\c"#), r#""a\"b\\c""#);
        assert_eq!(sparql_string("x\ny"), r#""x\ny""#);
    }
}
