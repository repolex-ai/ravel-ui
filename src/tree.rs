//! Packing one soul's memory tree for the page.
//!
//! ravel's memory index is a tree over time. Level 0 is a single memory,
//! placed in the hour it happened. Level k+1 summarizes a window of 2^k hours.
//! So the whole tree is a set of time spans stacked by level, which is
//! exactly what the page draws: time across, level up.
//!
//! It travels by column, not as one object per node. On lUX that is 34,000
//! memories and their summaries; as objects the field names alone would be
//! most of the payload.

use serde::Serialize;
use serde_json::{Map, Value};
use std::collections::HashMap;

#[derive(Serialize, Debug)]
pub struct Tree {
    pub soul: String,
    pub count: usize,
    /// Highest level present.
    pub top: u32,
    /// Seconds since the epoch. The span the whole tree covers.
    pub first: i64,
    pub last: i64,
    /// Parallel columns, sorted by level and then by start.
    pub ids: Vec<String>,
    pub level: Vec<u32>,
    pub start: Vec<i64>,
    pub end: Vec<i64>,
    /// Summary text, or null for a single memory (fetched when opened).
    pub text: Vec<Option<String>>,
    /// Rows dropped because a field could not be read. Counted, not hidden.
    pub unreadable: usize,
}

fn epoch(s: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(s).ok().map(|d| d.timestamp())
}

pub fn pack(soul: &str, shape: Vec<Map<String, Value>>, texts: Vec<Map<String, Value>>) -> Result<Tree, String> {
    let text_of: HashMap<String, String> = texts
        .into_iter()
        .filter_map(|r| {
            Some((
                r.get("id")?.as_str()?.to_string(),
                r.get("t")?.as_str()?.to_string(),
            ))
        })
        .collect();

    let mut rows: Vec<(u32, i64, i64, String)> = Vec::with_capacity(shape.len());
    let mut unreadable = 0;
    for r in shape {
        let get = |k: &str| r.get(k).and_then(Value::as_str);
        match (get("id"), get("l").and_then(|v| v.parse().ok()), get("ws").and_then(epoch), get("we").and_then(epoch)) {
            (Some(id), Some(l), Some(a), Some(b)) => rows.push((l, a, b, id.to_string())),
            _ => unreadable += 1,
        }
    }
    if rows.is_empty() && unreadable == 0 {
        return Err(format!("{soul} has no memory index yet — nothing in {}", super::raveld::MEMORY_GRAPH));
    }
    rows.sort();

    let n = rows.len();
    let mut t = Tree {
        soul: soul.to_string(),
        count: n,
        top: rows.iter().map(|r| r.0).max().unwrap_or(0),
        first: rows.iter().map(|r| r.1).min().unwrap_or(0),
        last: rows.iter().map(|r| r.2).max().unwrap_or(0),
        ids: Vec::with_capacity(n),
        level: Vec::with_capacity(n),
        start: Vec::with_capacity(n),
        end: Vec::with_capacity(n),
        text: Vec::with_capacity(n),
        unreadable,
    };
    for (l, a, b, id) in rows {
        t.text.push(if l > 0 { text_of.get(&id).cloned() } else { None });
        t.ids.push(id);
        t.level.push(l);
        t.start.push(a);
        t.end.push(b);
    }
    Ok(t)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn row(v: Value) -> Map<String, Value> {
        v.as_object().unwrap().clone()
    }

    #[test]
    fn packs_by_level_then_time_and_counts_what_it_cannot_read() {
        let shape = vec![
            row(json!({"id":"w1-2","l":"2","ws":"2026-04-03T16:00:00Z","we":"2026-04-03T20:00:00Z"})),
            row(json!({"id":"m2","l":"0","ws":"2026-04-03T19:00:00Z","we":"2026-04-03T20:00:00Z"})),
            row(json!({"id":"m1","l":"0","ws":"2026-04-03T18:00:00Z","we":"2026-04-03T19:00:00Z"})),
            row(json!({"id":"bad","l":"zero","ws":"2026-04-03T18:00:00Z","we":"x"})),
        ];
        let texts = vec![row(json!({"id":"w1-2","t":"a summary"}))];
        let t = pack("e3d71e", shape, texts).unwrap();
        assert_eq!(t.ids, ["m1", "m2", "w1-2"]);
        assert_eq!(t.level, [0, 0, 2]);
        assert_eq!(t.text, [None, None, Some("a summary".to_string())]);
        assert_eq!(t.top, 2);
        assert_eq!(t.end[2] - t.start[2], 4 * 3600);
        assert_eq!(t.unreadable, 1);
    }

    #[test]
    fn an_empty_index_is_an_error_not_an_empty_picture() {
        assert!(pack("9a3b27", vec![], vec![]).is_err());
    }
}
