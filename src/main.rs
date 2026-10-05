//! ravel-ui — a front door to every ravel soul on the machine.
//!
//! The data comes from `raveld`, the one ravel daemon, which holds every
//! soul's transcript store and memory index and answers for each by its short
//! id. This is the same shape as git-lex-ui over `gitlexd`: one daemon sees
//! every instance, and one page shows any of them.
//!
//! The browser only ever talks to this process, and this process only ever
//! asks raveld to READ. See `raveld.rs` for why that is enforced here rather
//! than trusted.

mod raveld;
mod tree;

use axum::{
    Json, Router,
    extract::{Path as AxPath, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    routing::get,
};
use clap::Parser;
use raveld::{MEMORY_GRAPH, RAVEL, Raveld, sparql_string, valid_id};
use serde_json::{Value, json};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Parser, Debug)]
#[command(name = "ravel-ui", about = "A front door to every ravel soul on the machine", version)]
struct Args {
    /// Port for the front door itself. git-lex-ui holds 8888.
    #[arg(long, default_value = "8889")]
    port: u16,

    /// Where `raveld` listens.
    #[arg(long, default_value_t = raveld::DAEMON_PORT)]
    daemon_port: u16,

    /// Do not open a browser tab on startup.
    #[arg(long)]
    no_open: bool,

    /// Frontend build directory, read on every request so it can be edited
    /// live. Defaults to ./web/dist.
    #[arg(long)]
    web: Option<PathBuf>,
}

struct AppState {
    raveld: Raveld,
    web_dir: PathBuf,
}

type S = State<Arc<AppState>>;

#[tokio::main]
async fn main() {
    let args = Args::parse();
    let web_dir = args.web.clone().unwrap_or_else(default_web_dir);
    let state = Arc::new(AppState { raveld: Raveld::new(args.daemon_port), web_dir: web_dir.clone() });

    let app = Router::new()
        .route("/", get(index))
        .route("/api/health", get(api_health))
        .route("/api/souls", get(api_souls))
        .route("/api/souls/{id}/stats", get(api_stats))
        .route("/api/souls/{id}/tree", get(api_tree))
        .route("/api/souls/{id}/node/{node}", get(api_node))
        .route("/api/souls/{id}/search", get(api_search))
        .route("/api/souls/{id}/wake", get(api_wake))
        .route("/{*asset}", get(static_asset))
        .with_state(Arc::clone(&state));

    // Fixed port, and it fails loudly if taken. A front door that quietly
    // moves is a front door nobody can find.
    let addr = format!("127.0.0.1:{}", args.port);
    let listener = match tokio::net::TcpListener::bind(&addr).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("ravel-ui: could not bind {addr}: {e}");
            eprintln!("Something already holds port {}. Stop it, or pass --port.", args.port);
            std::process::exit(1);
        }
    };
    let url = format!("http://{addr}");
    println!("ravel-ui listening on {url}");
    println!("Frontend served from {}", web_dir.display());
    println!("Data feed: raveld on {}", state.raveld.port_url());
    if !args.no_open {
        let _ = open::that_detached(&url);
    }
    let serve = axum::serve(listener, app).with_graceful_shutdown(async {
        let _ = tokio::signal::ctrl_c().await;
        println!();
    });
    if let Err(e) = serve.await {
        eprintln!("server error: {e}");
    }
}

fn default_web_dir() -> PathBuf {
    let cwd = std::env::current_dir().unwrap_or_default();
    for c in [cwd.join("web/dist"), cwd.join("dist")] {
        if c.is_dir() {
            return c;
        }
    }
    cwd.join("web/dist")
}

fn err(code: StatusCode, msg: impl Into<String>) -> Response {
    (code, msg.into()).into_response()
}

fn bad_gateway(msg: String) -> Response {
    err(StatusCode::BAD_GATEWAY, msg)
}

fn check(id: &str) -> Result<(), Response> {
    if valid_id(id) { Ok(()) } else { Err(err(StatusCode::BAD_REQUEST, format!("`{id}` is not an id"))) }
}

// --------------------------------------------------------------------- api

/// The daemon's own health, passed through, or the reason it cannot be had.
/// Asked fresh every time: a page left open for an hour must find out its
/// feed died.
async fn api_health(State(s): S) -> Response {
    match s.raveld.get("/health").await {
        Ok(mut h) => {
            // raveld's health includes the memory index's API spend. That is
            // not this page's business, so it never reaches the browser.
            if let Some(o) = h.as_object_mut() {
                o.remove("memory");
            }
            Json(json!({ "raveld": h, "raveld_url": s.raveld.port_url() })).into_response()
        }
        Err(e) => Json(json!({ "raveld": null, "raveld_url": s.raveld.port_url(), "error": e })).into_response(),
    }
}

/// Every soul raveld holds, with what its memory index contains.
///
/// The counts come from one aggregate query per soul, run together. They are
/// reported as `null`, not zero, when the query fails: a soul whose store
/// cannot be read and a soul with nothing indexed yet are different answers.
async fn api_souls(State(s): S) -> Response {
    let list = match s.raveld.get("/souls").await {
        Ok(v) => v,
        Err(e) => return bad_gateway(e),
    };
    let Some(souls) = list.as_array() else {
        return bad_gateway("raveld's /souls is not a list".into());
    };
    let q = format!(
        "PREFIX r: <{RAVEL}>
         SELECT (SUM(IF(?l = 0, 1, 0)) AS ?memories) (SUM(IF(?l > 0, 1, 0)) AS ?summaries)
                (MIN(?ws) AS ?first) (MAX(?we) AS ?last)
         WHERE {{ GRAPH <{MEMORY_GRAPH}> {{ ?m r:memoryLevel ?l ; r:windowStart ?ws ; r:windowEnd ?we }} }}"
    );
    let jobs = souls.iter().map(|soul| {
        let id = soul.get("id").and_then(Value::as_str).unwrap_or_default().to_string();
        let q = q.clone();
        let rv = s.raveld.clone();
        async move {
            if !valid_id(&id) {
                return Err("unusable id".to_string());
            }
            rv.select(&id, &q).await.map(|rows| rows.into_iter().next().unwrap_or_default())
        }
    });
    let counts = futures_util::future::join_all(jobs).await;
    let out: Vec<Value> = souls
        .iter()
        .zip(counts)
        .map(|(soul, c)| {
            let path = soul.get("path").and_then(Value::as_str).unwrap_or_default();
            let name = path.rsplit('/').next().unwrap_or(path);
            let num = |row: &serde_json::Map<String, Value>, k: &str| {
                row.get(k).and_then(Value::as_str).and_then(|v| v.parse::<u64>().ok()).or(Some(0))
            };
            let (memories, summaries, first, last, index_error) = match &c {
                Ok(row) => (
                    num(row, "memories"),
                    num(row, "summaries"),
                    row.get("first").cloned().unwrap_or(Value::Null),
                    row.get("last").cloned().unwrap_or(Value::Null),
                    Value::Null,
                ),
                Err(e) => (None, None, Value::Null, Value::Null, Value::from(e.clone())),
            };
            json!({
                "id": soul.get("id"),
                "name": name,
                "path": path,
                "last_sync": soul.get("last_sync"),
                "last_error": soul.get("last_error"),
                "memories": memories,
                "summaries": summaries,
                "first": first,
                "last": last,
                "index_error": index_error,
            })
        })
        .collect();
    Json(out).into_response()
}

async fn api_stats(State(s): S, AxPath(id): AxPath<String>) -> Response {
    if let Err(r) = check(&id) {
        return r;
    }
    match s.raveld.get(&format!("/souls/{id}/stats")).await {
        Ok(v) => Json(v).into_response(),
        Err(e) => bad_gateway(e),
    }
}

/// The whole memory tree of one soul, packed by column. See `tree.rs`.
async fn api_tree(State(s): S, AxPath(id): AxPath<String>) -> Response {
    if let Err(r) = check(&id) {
        return r;
    }
    let shape = format!(
        "PREFIX r: <{RAVEL}>
         SELECT ?id ?l ?ws ?we WHERE {{ GRAPH <{MEMORY_GRAPH}> {{
             ?m r:memoryId ?id ; r:memoryLevel ?l ; r:windowStart ?ws ; r:windowEnd ?we }} }}"
    );
    // Summaries carry their text up front, so hovering the upper levels needs
    // no round trip. Single memories do not: on lUX that would be 34,000
    // texts shipped to draw a picture of their timing.
    let texts = format!(
        "PREFIX r: <{RAVEL}>
         SELECT ?id ?t WHERE {{ GRAPH <{MEMORY_GRAPH}> {{
             ?m r:memoryId ?id ; r:memoryLevel ?l ; r:memoryText ?t FILTER(?l > 0) }} }}"
    );
    let (shape, texts) = tokio::join!(s.raveld.select(&id, &shape), s.raveld.select(&id, &texts));
    let shape = match shape {
        Ok(r) => r,
        Err(e) => return bad_gateway(e),
    };
    let texts = texts.unwrap_or_default();
    match tree::pack(&id, shape, texts) {
        Ok(t) => Json(t).into_response(),
        Err(e) => bad_gateway(e),
    }
}

/// One node: its own text and window, plus raveld's opening of it — the
/// children of a window, or the source turns of a memory.
async fn api_node(State(s): S, AxPath((id, node)): AxPath<(String, String)>) -> Response {
    if let Err(r) = check(&id).and(check(&node)) {
        return r;
    }
    let me = format!(
        "PREFIX r: <{RAVEL}>
         SELECT ?l ?ws ?we ?t ?model WHERE {{ GRAPH <{MEMORY_GRAPH}> {{
             ?m r:memoryId {lit} ; r:memoryLevel ?l ; r:windowStart ?ws ; r:windowEnd ?we .
             OPTIONAL {{ ?m r:memoryText ?t }} OPTIONAL {{ ?m r:memoryModel ?model }} }} }} LIMIT 1",
        lit = sparql_string(&node)
    );
    let open_path = format!("/souls/{id}/memory/{node}");
    let (me, open) = tokio::join!(s.raveld.select(&id, &me), s.raveld.get(&open_path));
    let me = match me {
        Ok(rows) => rows.into_iter().next(),
        Err(e) => return bad_gateway(e),
    };
    let Some(me) = me else {
        return err(StatusCode::NOT_FOUND, format!("no memory or summary `{node}` in {id}"));
    };
    Json(json!({
        "id": node,
        "level": me.get("l").and_then(Value::as_str).and_then(|v| v.parse::<u32>().ok()),
        "from": me.get("ws"),
        "to": me.get("we"),
        "text": me.get("t"),
        "model": me.get("model"),
        // raveld's own answer, or the reason it had none. A window that has
        // not been summarized yet still opens to its children.
        "open": open.unwrap_or_else(|e| json!({ "error": e })),
    }))
    .into_response()
}

/// What the soul wakes up with: raveld's own startup view, the same lines
/// `ravel memory` prints. Fine detail near now, coarser summaries further
/// back. Passed through unchanged except for a size count, so the page shows
/// the view the soul actually gets and not a reconstruction of it.
async fn api_wake(State(s): S, AxPath(id): AxPath<String>) -> Response {
    if let Err(r) = check(&id) {
        return r;
    }
    match s.raveld.get(&format!("/souls/{id}/memory")).await {
        Ok(mut v) => {
            let chars: usize = v
                .get("lines")
                .and_then(Value::as_array)
                .map(|ls| ls.iter().filter_map(|l| l.get("text").and_then(Value::as_str)).map(|t| t.chars().count()).sum())
                .unwrap_or(0);
            if let Some(o) = v.as_object_mut() {
                o.insert("chars".into(), Value::from(chars));
            }
            Json(v).into_response()
        }
        Err(e) => bad_gateway(e),
    }
}

#[derive(serde::Deserialize)]
struct SearchQuery {
    q: String,
}

/// Memories and summaries whose text contains the words, oldest first.
async fn api_search(State(s): S, AxPath(id): AxPath<String>, Query(q): Query<SearchQuery>) -> Response {
    if let Err(r) = check(&id) {
        return r;
    }
    let needle = q.q.trim().to_lowercase();
    if needle.len() < 2 {
        return Json(json!({ "query": needle, "hits": [], "truncated": false })).into_response();
    }
    const LIMIT: usize = 400;
    let query = format!(
        "PREFIX r: <{RAVEL}>
         SELECT ?id ?l ?ws ?t WHERE {{ GRAPH <{MEMORY_GRAPH}> {{
             ?m r:memoryId ?id ; r:memoryLevel ?l ; r:windowStart ?ws ; r:memoryText ?t
             FILTER(CONTAINS(LCASE(?t), {lit})) }} }}
         ORDER BY ?ws ?l LIMIT {cap}",
        lit = sparql_string(&needle),
        cap = LIMIT + 1
    );
    match s.raveld.select(&id, &query).await {
        Ok(mut rows) => {
            // One extra row asked for, so "exactly the limit" and "more than
            // the limit" are told apart instead of both reading as complete.
            let truncated = rows.len() > LIMIT;
            rows.truncate(LIMIT);
            let hits: Vec<Value> = rows
                .into_iter()
                .map(|r| {
                    json!({
                        "id": r.get("id"),
                        "level": r.get("l").and_then(Value::as_str).and_then(|v| v.parse::<u32>().ok()),
                        "from": r.get("ws"),
                        "text": r.get("t"),
                    })
                })
                .collect();
            Json(json!({ "query": needle, "hits": hits, "truncated": truncated })).into_response()
        }
        Err(e) => bad_gateway(e),
    }
}

// ------------------------------------------------------------------ static

async fn index(State(s): S) -> Response {
    match std::fs::read_to_string(s.web_dir.join("index.html")) {
        Ok(html) => Html(html).into_response(),
        Err(_) => Html(format!(
            "<pre style=\"font:14px ui-monospace,monospace;padding:2rem\">\
             ravel-ui is running, but the frontend is not built.\n\n\
             Looked in: {}\n\n\
             Build it with:  cd web &amp;&amp; npm install &amp;&amp; npm run build\n\
             Or point elsewhere with:  ravel-ui --web &lt;dir&gt;\n</pre>",
            s.web_dir.display()
        ))
        .into_response(),
    }
}

async fn static_asset(State(s): S, AxPath(asset): AxPath<String>) -> Response {
    let rel = PathBuf::from(asset.trim_start_matches('/'));
    let full = s.web_dir.join(&rel);
    let root = s.web_dir.canonicalize().unwrap_or_else(|_| s.web_dir.clone());
    let canon = match full.canonicalize() {
        Ok(c) => c,
        Err(_) => return err(StatusCode::NOT_FOUND, "not found"),
    };
    if !canon.starts_with(&root) {
        return err(StatusCode::FORBIDDEN, "outside the web root");
    }
    let ct = match canon.extension().and_then(|e| e.to_str()) {
        Some("js") | Some("mjs") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("html") => "text/html; charset=utf-8",
        Some("json") => "application/json",
        Some("svg") => "image/svg+xml",
        Some("woff2") => "font/woff2",
        _ => "application/octet-stream",
    };
    match std::fs::read(&canon) {
        Ok(bytes) => ([(axum::http::header::CONTENT_TYPE, ct)], bytes).into_response(),
        Err(_) => err(StatusCode::NOT_FOUND, "not found"),
    }
}
