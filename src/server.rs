//! Local web front end: serves the UI and a small JSON API over the audit pipeline.
//!
//! Built on `std::net` alone (no async runtime, no extra dependencies) and bound to
//! loopback by default, in keeping with the tool's offline design: it is meant to be
//! opened on the analyst's own machine, never exposed on an OT network.
//!
//! Routes (every response is sent with `Connection: close`):
//!   GET    /  /dashboard  /related-work   HTML pages (assets are embedded at build time)
//!   GET    /health                        liveness probe
//!   GET    /api/samples                   built-in demo keys, one per defect class
//!   POST   /api/audit                     audit the key(s) in the body and store them
//!   GET    /api/keys                      summaries of stored keys
//!   DELETE /api/keys                      clear stored keys
//!   GET    /api/report/{id}               operator report for one stored key
//!   POST   /api/batch-gcd                 tree batch GCD across all stored keys
//!   POST   /api/bundle                    research bundle: benchmark + ECM yield study
//!
//! Like the CLI, the API only ever returns operator reports: no endpoint can carry a
//! recovered private factor.

use crate::benchmark::ecm_study::run_ecm_study;
use crate::benchmark::evaluate::run_benchmark;
use crate::benchmark::generate::{generate, CorpusSpec};
use crate::benchmark::DefectClass;
use crate::detectors::{self, batch_gcd, DetectorConfig, Signal};
use crate::key::{self, RsaPublicKey};
use crate::mathutil::random_prime_seeded;
use crate::{report, scoring};
use serde::Serialize;
use serde_json::{json, Value};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

const MAX_HEADER_BYTES: u64 = 16 * 1024;
const MAX_BODY_BYTES: usize = 2 * 1024 * 1024;
const MAX_STORED_KEYS: usize = 1000;

pub struct Response {
    pub status: u16,
    pub content_type: &'static str,
    pub body: Vec<u8>,
}

impl Response {
    fn json(status: u16, value: &impl Serialize) -> Self {
        Response {
            status,
            content_type: "application/json",
            body: serde_json::to_vec_pretty(value).unwrap_or_else(|_| b"{}".to_vec()),
        }
    }

    fn error(status: u16, message: impl Into<String>) -> Self {
        Self::json(status, &json!({ "error": message.into() }))
    }

    fn text(content_type: &'static str, body: &'static str) -> Self {
        Response { status: 200, content_type, body: body.as_bytes().to_vec() }
    }

    /// The body as UTF-8 text (convenient for tests and logging).
    pub fn body_text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }
}

/// A built-in demo key (or keys) for the dashboard.
#[derive(Clone, Serialize)]
pub struct Sample {
    pub id: &'static str,
    pub label: &'static str,
    pub expect: &'static str,
    pub description: &'static str,
    pub keys: Vec<String>,
}

struct StoredKey {
    id: usize,
    key: RsaPublicKey,
    signals: Vec<Signal>,
}

#[derive(Default)]
struct Store {
    next_id: usize,
    keys: Vec<StoredKey>,
}

/// Shared server state: the stored keys and the lazily generated sample set.
#[derive(Default)]
pub struct AppState {
    store: Mutex<Store>,
    samples: OnceLock<Vec<Sample>>,
}

impl AppState {
    pub fn new() -> Self {
        Self::default()
    }

    fn samples(&self) -> &[Sample] {
        self.samples.get_or_init(build_samples)
    }
}

/* ------------------------------------------------------------------ assets */

fn asset(path: &str) -> Option<Response> {
    let (content_type, body) = match path {
        "/" | "/index.html" => ("text/html; charset=utf-8", include_str!("../web/index.html")),
        "/dashboard" | "/dashboard.html" => ("text/html; charset=utf-8", include_str!("../web/dashboard.html")),
        "/related-work" | "/related-work.html" => ("text/html; charset=utf-8", include_str!("../web/related-work.html")),
        "/styles.css" => ("text/css; charset=utf-8", include_str!("../web/styles.css")),
        "/console.css" => ("text/css; charset=utf-8", include_str!("../web/console.css")),
        "/iso.js" => ("text/javascript; charset=utf-8", include_str!("../web/iso.js")),
        "/scenes.js" => ("text/javascript; charset=utf-8", include_str!("../web/scenes.js")),
        "/app.js" => ("text/javascript; charset=utf-8", include_str!("../web/app.js")),
        "/dashboard.js" => ("text/javascript; charset=utf-8", include_str!("../web/dashboard.js")),
        "/favicon.svg" => ("image/svg+xml", include_str!("../web/favicon.svg")),
        _ => return None,
    };
    Some(Response::text(content_type, body))
}

/* ------------------------------------------------------------------ router */

/// Route one request. Pure (no I/O), so the whole API is testable without sockets.
pub fn handle(state: &AppState, method: &str, target: &str, body: &[u8]) -> Response {
    let path = target.split('?').next().unwrap_or("/");
    match (method, path) {
        ("GET", "/health") => health(state),
        ("GET", "/api/samples") => Response::json(200, &state.samples()),
        ("POST", "/api/audit") => audit(state, body),
        ("GET", "/api/keys") => list_keys(state),
        ("DELETE", "/api/keys") => clear_keys(state),
        ("POST", "/api/batch-gcd") => batch(state),
        ("POST", "/api/bundle") => bundle(body),
        ("GET", p) if p.starts_with("/api/report/") => report_for(state, &p["/api/report/".len()..]),
        ("GET", p) => asset(p).unwrap_or_else(|| Response::error(404, format!("no route for {p}"))),
        (_, p) if p.starts_with("/api/") || p == "/health" => Response::error(405, format!("{method} not allowed on {p}")),
        (_, p) => Response::error(404, format!("no route for {method} {p}")),
    }
}

fn health(state: &AppState) -> Response {
    let stored = state.store.lock().map(|s| s.keys.len()).unwrap_or(0);
    Response::json(
        200,
        &json!({
            "status": "ok",
            "tool": format!("rsashield {}", env!("CARGO_PKG_VERSION")),
            "mode": "offline",
            "stored_keys": stored,
        }),
    )
}

/// Audit every key in the body (PEM blocks and/or hex `n:e` lines) and store them.
fn audit(state: &AppState, body: &[u8]) -> Response {
    let text = match std::str::from_utf8(body) {
        Ok(t) if !t.trim().is_empty() => t,
        Ok(_) => return Response::error(400, "empty request: paste a PEM public key or a hex modulus:exponent pair"),
        Err(_) => return Response::error(400, "request body is not UTF-8 text"),
    };

    let mut records = key::parse_many(text, "dashboard");
    if records.is_empty() {
        // nothing record-shaped: report the precise reason for the whole input
        records.push(key::parse_any(text, "dashboard"));
    }
    let mut parsed = Vec::new();
    let mut errors = Vec::new();
    for (i, r) in records.into_iter().enumerate() {
        match r {
            Ok(k) => parsed.push(k),
            Err(e) => errors.push(json!({ "record": i, "error": e.to_string() })),
        }
    }
    if parsed.is_empty() {
        let message = errors
            .first()
            .and_then(|e| e["error"].as_str())
            .unwrap_or("no key found")
            .to_string();
        return Response::json(400, &json!({ "error": message, "errors": errors }));
    }
    {
        let store = state.store.lock().expect("store lock");
        if store.keys.len() + parsed.len() > MAX_STORED_KEYS {
            return Response::error(409, format!("store limit of {MAX_STORED_KEYS} keys reached; clear stored keys first"));
        }
    }

    // run the detectors outside the lock; they can take a moment per key
    let cfg = DetectorConfig::default();
    let audited: Vec<(RsaPublicKey, Vec<Signal>)> = parsed
        .into_iter()
        .map(|k| {
            let signals = detectors::run_single_key(&k, &cfg);
            (k, signals)
        })
        .collect();

    let mut store = state.store.lock().expect("store lock");
    let mut results = Vec::new();
    for (k, signals) in audited {
        store.next_id += 1;
        let id = store.next_id;
        results.push(json!({ "id": id, "report": operator_report(&k, &signals) }));
        store.keys.push(StoredKey { id, key: k, signals });
    }
    Response::json(200, &json!({ "results": results, "errors": errors }))
}

fn operator_report(k: &RsaPublicKey, signals: &[Signal]) -> report::OperatorReport {
    let assessment = scoring::assess(signals);
    report::operator_report(k, &assessment, signals)
}

fn list_keys(state: &AppState) -> Response {
    let store = state.store.lock().expect("store lock");
    let rows: Vec<Value> = store
        .keys
        .iter()
        .map(|s| {
            let a = scoring::assess(&s.signals);
            json!({
                "id": s.id,
                "key": s.key.to_string(),
                "modulus_bits": s.key.modulus_bits(),
                "classification": a.classification.as_str(),
                "risk_score": a.risk_score,
                "failure_modes": a.failure_modes.iter().map(|m| format!("{m:?}")).collect::<Vec<_>>(),
            })
        })
        .collect();
    Response::json(200, &json!({ "keys": rows }))
}

fn clear_keys(state: &AppState) -> Response {
    let mut store = state.store.lock().expect("store lock");
    let cleared = store.keys.len();
    store.keys.clear();
    Response::json(200, &json!({ "cleared": cleared }))
}

fn report_for(state: &AppState, id: &str) -> Response {
    let Ok(id) = id.parse::<usize>() else {
        return Response::error(400, "report id must be a number");
    };
    let store = state.store.lock().expect("store lock");
    match store.keys.iter().find(|s| s.id == id) {
        Some(s) => Response::json(200, &operator_report(&s.key, &s.signals)),
        None => Response::error(404, format!("no stored key with id {id}")),
    }
}

/// Run tree batch GCD over every stored key and fold any shared-prime findings back
/// into those keys' signals, so their reports reflect the corpus-level result.
fn batch(state: &AppState) -> Response {
    let mut store = state.store.lock().expect("store lock");
    let n = store.keys.len();
    if n < 2 {
        return Response::error(400, "batch GCD needs at least two stored keys");
    }
    let keys: Vec<RsaPublicKey> = store.keys.iter().map(|s| s.key.clone()).collect();
    let result = batch_gcd::run_tree(&keys);

    for s in store.keys.iter_mut() {
        s.signals.retain(|sig| sig.detector != "batch_gcd");
    }
    let mut shared = Vec::new();
    for (idx, sig) in result.signals {
        let stored = &mut store.keys[idx];
        shared.push(json!({ "id": stored.id, "key": stored.key.to_string(), "detail": sig.detail }));
        stored.signals.push(sig);
    }
    Response::json(
        200,
        &json!({
            "keys": n,
            "pairs": n * (n - 1) / 2,
            "keys_affected": result.keys_affected,
            "shared": shared,
            "elapsed_ms": result.elapsed_ms,
        }),
    )
}

/// The research bundle: the labelled benchmark plus the ECM marginal-yield study.
/// Body (optional JSON): `{"seed": 1, "bits": 512, "quick": false}`.
fn bundle(body: &[u8]) -> Response {
    let opts: Value = if body.iter().all(u8::is_ascii_whitespace) {
        json!({})
    } else {
        match serde_json::from_slice(body) {
            Ok(v) => v,
            Err(e) => return Response::error(400, format!("bundle options must be JSON: {e}")),
        }
    };
    let seed = opts["seed"].as_u64().unwrap_or(1);
    let bits = opts["bits"].as_u64().unwrap_or(512);
    let quick = opts["quick"].as_bool().unwrap_or(false);
    if !(256..=1024).contains(&bits) {
        return Response::error(400, "bits must be between 256 and 1024");
    }

    let start = Instant::now();
    let spec = CorpusSpec { bits, ..Default::default() };
    let (metrics, _corpus) = run_benchmark(&spec, seed);

    let (factor_sizes, budgets, per_cell): (&[u64], &[(u64, u32)], usize) = if quick {
        (&[18, 22], &[(500, 10), (2_000, 10)], 3)
    } else {
        (&[18, 22, 26, 30], &[(500, 10), (2_000, 10), (5_000, 16), (10_000, 16)], 6)
    };
    let study = run_ecm_study(bits, seed, factor_sizes, budgets, per_cell);

    Response::json(
        200,
        &json!({
            "seed": seed,
            "bits": bits,
            "metrics": metrics,
            "ecm_study": { "modulus_bits": study.modulus_bits, "rows": study.rows },
            "elapsed_ms": start.elapsed().as_millis(),
        }),
    )
}

/* ----------------------------------------------------------------- samples */

fn hex_pair(k: &RsaPublicKey) -> String {
    format!("{}:{}", k.n.to_str_radix(16), k.e.to_str_radix(16))
}

/// One seeded synthetic key per defect class (public halves only), plus a legacy
/// 1024-bit key and a malformed PEM for the error path.
fn build_samples() -> Vec<Sample> {
    let corpus = generate(&CorpusSpec::default(), 7);
    let of = |class: DefectClass, count: usize| -> Vec<String> {
        corpus
            .truth
            .iter()
            .filter(|t| t.class == class)
            .take(count)
            .map(|t| hex_pair(&corpus.keys[t.index]))
            .collect()
    };
    let legacy = {
        let p = random_prime_seeded(512, 41);
        let q = random_prime_seeded(512, 42);
        RsaPublicKey::new(&p * &q, 65537u32.into(), "legacy-1024")
    };

    vec![
        Sample {
            id: "small-factor",
            label: "Small factor",
            expect: "FACTOR_RECOVERED",
            description: "One prime is tiny; trial division splits the modulus.",
            keys: of(DefectClass::SmallFactor, 1),
        },
        Sample {
            id: "close-primes",
            label: "Close primes",
            expect: "FACTOR_RECOVERED",
            description: "p and q drawn almost equal; Fermat factors it in a step.",
            keys: of(DefectClass::ClosePrime, 1),
        },
        Sample {
            id: "small-d",
            label: "Small private exponent",
            expect: "FACTOR_RECOVERED",
            description: "d chosen too small; Wiener's attack recovers it.",
            keys: of(DefectClass::SmallD, 1),
        },
        Sample {
            id: "unbalanced",
            label: "Unbalanced factor",
            expect: "FACTOR_RECOVERED",
            description: "An ~18-bit factor beyond trial division, found by bounded ECM.",
            keys: of(DefectClass::Unbalanced, 1),
        },
        Sample {
            id: "shared-prime",
            label: "Shared prime (2 keys)",
            expect: "FACTOR_RECOVERED after batch GCD",
            description: "Two keys reuse a prime; each looks clean alone until batch GCD runs.",
            keys: of(DefectClass::SharedPrime, 2),
        },
        Sample {
            id: "healthy",
            label: "Normal 512-bit",
            expect: "NO_DEFECT_FOUND",
            description: "Independent primes; only the undersized-modulus advisory fires.",
            keys: of(DefectClass::Healthy, 1),
        },
        Sample {
            id: "legacy-1024",
            label: "Legacy 1024-bit",
            expect: "NO_DEFECT_FOUND",
            description: "Sound primes but below the 2048-bit policy floor.",
            keys: vec![hex_pair(&legacy)],
        },
        Sample {
            id: "bad-pem",
            label: "Malformed PEM",
            expect: "400 parse error",
            description: "Not a key at all; the API rejects it with a clear error.",
            keys: vec!["-----BEGIN PUBLIC KEY-----\nTm90IGEga2V5IGF0IGFsbA==\n-----END PUBLIC KEY-----".to_string()],
        },
    ]
}

/* -------------------------------------------------------------------- HTTP */

/// Bind `addr` and serve until the process exits.
pub fn serve(addr: &str) -> io::Result<()> {
    let listener = TcpListener::bind(addr)?;
    let local = listener.local_addr()?;
    eprintln!("rsashield {} serving http://{local}", env!("CARGO_PKG_VERSION"));
    if !local.ip().is_loopback() {
        eprintln!("warning: listening beyond loopback; keep this off OT networks (see docs/THREAT_MODEL.md)");
    }
    run(listener, Arc::new(AppState::new()))
}

/// Serve connections from an already-bound listener (one thread per connection).
pub fn run(listener: TcpListener, state: Arc<AppState>) -> io::Result<()> {
    for stream in listener.incoming() {
        let Ok(stream) = stream else { continue };
        let state = Arc::clone(&state);
        thread::spawn(move || {
            let _ = handle_connection(stream, &state);
        });
    }
    Ok(())
}

fn handle_connection(mut stream: TcpStream, state: &AppState) -> io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(15)))?;
    let mut reader = BufReader::new(stream.try_clone()?);

    let (method, target, content_length) = {
        let mut head = (&mut reader).take(MAX_HEADER_BYTES);
        let mut request_line = String::new();
        if head.read_line(&mut request_line)? == 0 {
            return Ok(());
        }
        let mut parts = request_line.split_whitespace();
        let (Some(method), Some(target)) = (parts.next(), parts.next()) else {
            return write_response(&mut stream, &Response::error(400, "malformed request line"));
        };
        let (method, target) = (method.to_string(), target.to_string());

        let mut content_length = 0usize;
        loop {
            let mut line = String::new();
            if head.read_line(&mut line)? == 0 {
                return write_response(&mut stream, &Response::error(431, "request headers too large or truncated"));
            }
            let line = line.trim_end();
            if line.is_empty() {
                break;
            }
            if let Some((name, value)) = line.split_once(':') {
                if name.trim().eq_ignore_ascii_case("content-length") {
                    match value.trim().parse() {
                        Ok(n) => content_length = n,
                        Err(_) => return write_response(&mut stream, &Response::error(400, "invalid Content-Length")),
                    }
                }
            }
        }
        (method, target, content_length)
    };

    if content_length > MAX_BODY_BYTES {
        return write_response(&mut stream, &Response::error(413, "request body too large (limit 2 MiB)"));
    }
    let mut body = vec![0u8; content_length];
    reader.read_exact(&mut body)?;

    let response = handle(state, &method, &target, &body);
    write_response(&mut stream, &response)
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        405 => "Method Not Allowed",
        409 => "Conflict",
        413 => "Payload Too Large",
        431 => "Request Header Fields Too Large",
        _ => "Internal Server Error",
    }
}

fn write_response(stream: &mut TcpStream, r: &Response) -> io::Result<()> {
    let head = format!(
        "HTTP/1.1 {} {}\r\n\
         Content-Type: {}\r\n\
         Content-Length: {}\r\n\
         Cache-Control: no-store\r\n\
         X-Content-Type-Options: nosniff\r\n\
         Referrer-Policy: no-referrer\r\n\
         Connection: close\r\n\r\n",
        r.status,
        reason(r.status),
        r.content_type,
        r.body.len()
    );
    stream.write_all(head.as_bytes())?;
    stream.write_all(&r.body)?;
    stream.flush()
}
