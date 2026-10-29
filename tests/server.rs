//! Web API tests. These mirror the original prototype's "live checks" (health and
//! pages, one sample per detector, bad PEM -> 400, batch GCD over stored keys, report
//! JSON, research bundle) and add the redaction and HTTP-plumbing guarantees.

use rsashield::benchmark::generate::{generate, CorpusSpec};
use rsashield::benchmark::DefectClass;
use rsashield::server::{self, AppState};
use serde_json::Value;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::thread;

fn call(state: &AppState, method: &str, path: &str, body: &str) -> (u16, Value) {
    let r = server::handle(state, method, path, body.as_bytes());
    let json = serde_json::from_slice(&r.body).unwrap_or(Value::Null);
    (r.status, json)
}

fn samples(state: &AppState) -> Vec<Value> {
    let (status, v) = call(state, "GET", "/api/samples", "");
    assert_eq!(status, 200);
    v.as_array().expect("samples array").clone()
}

fn sample_keys(all: &[Value], id: &str) -> String {
    let s = all.iter().find(|s| s["id"] == id).unwrap_or_else(|| panic!("sample {id} missing"));
    s["keys"].as_array().unwrap().iter().map(|k| k.as_str().unwrap()).collect::<Vec<_>>().join("\n")
}

fn hex_pair(n_hex: &str, e_hex: &str) -> String {
    format!("{n_hex}:{e_hex}")
}

#[test]
fn health_and_pages_return_200() {
    let state = AppState::new();
    for path in ["/health", "/", "/dashboard", "/related-work", "/styles.css", "/console.css", "/dashboard.js", "/favicon.svg"] {
        let r = server::handle(&state, "GET", path, b"");
        assert_eq!(r.status, 200, "{path} should be served");
    }
    let (_, health) = call(&state, "GET", "/health", "");
    assert_eq!(health["status"], "ok");
    assert_eq!(health["mode"], "offline");
}

#[test]
fn unknown_routes_and_methods_are_rejected() {
    let state = AppState::new();
    assert_eq!(server::handle(&state, "GET", "/nope", b"").status, 404);
    assert_eq!(server::handle(&state, "GET", "/../Cargo.toml", b"").status, 404);
    assert_eq!(server::handle(&state, "PUT", "/api/audit", b"").status, 405);
    assert_eq!(server::handle(&state, "GET", "/api/report/abc", b"").status, 400);
    assert_eq!(server::handle(&state, "GET", "/api/report/99", b"").status, 404);
}

#[test]
fn each_sample_is_caught_by_its_detector() {
    let state = AppState::new();
    let all = samples(&state);
    for (id, detector) in [("small-factor", "small_factor"), ("close-primes", "fermat"), ("small-d", "wiener"), ("unbalanced", "ecm")] {
        let (status, v) = call(&state, "POST", "/api/audit", &sample_keys(&all, id));
        assert_eq!(status, 200, "{id}");
        let report = &v["results"][0]["report"];
        assert_eq!(report["classification"], "FACTOR_RECOVERED", "{id}");
        let fired = report["signals"]
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s["detector"] == detector && s["verdict"] == "FactorRecovered");
        assert!(fired, "{id} should be recovered by {detector}");
    }
    for id in ["healthy", "legacy-1024"] {
        let (status, v) = call(&state, "POST", "/api/audit", &sample_keys(&all, id));
        assert_eq!(status, 200, "{id}");
        assert_eq!(v["results"][0]["report"]["classification"], "NO_DEFECT_FOUND", "{id}");
    }
}

#[test]
fn bad_pem_is_a_400_with_a_clear_parse_error() {
    let state = AppState::new();
    let all = samples(&state);
    let (status, v) = call(&state, "POST", "/api/audit", &sample_keys(&all, "bad-pem"));
    assert_eq!(status, 400);
    assert!(v["error"].as_str().unwrap().contains("key parse error"), "got {v}");

    let (status, _) = call(&state, "POST", "/api/audit", "   ");
    assert_eq!(status, 400);
    let (status, v) = call(&state, "POST", "/api/audit", "definitely not a key");
    assert_eq!(status, 400);
    assert!(v["error"].as_str().unwrap().contains("neither a PEM block nor a hex"));
}

#[test]
fn api_responses_never_contain_private_factors() {
    let corpus = generate(&CorpusSpec::default(), 7);
    let t = corpus.truth.iter().find(|t| t.class == DefectClass::SmallFactor).unwrap();
    let state = AppState::new();
    let r = server::handle(&state, "POST", "/api/audit", hex_pair(&t.n_hex, &t.e_hex).as_bytes());
    assert_eq!(r.status, 200);
    let id = serde_json::from_slice::<Value>(&r.body).unwrap()["results"][0]["id"].as_u64().unwrap();
    let report = server::handle(&state, "GET", &format!("/api/report/{id}"), b"").body_text();
    let keys = server::handle(&state, "GET", "/api/keys", b"").body_text();
    for body in [r.body_text(), report, keys] {
        assert!(body.contains("FACTOR_RECOVERED"));
        for secret in [&t.p_hex, &t.q_hex] {
            assert!(!body.to_lowercase().contains(&secret.to_lowercase()), "a private factor leaked");
        }
    }
}

#[test]
fn batch_gcd_over_six_stored_healthy_keys_finds_nothing() {
    // the prototype's check: 6 stored keys -> 15 pairs, 0 shared factors
    let corpus = generate(&CorpusSpec::default(), 3);
    let healthy: Vec<String> = corpus
        .truth
        .iter()
        .filter(|t| t.class == DefectClass::Healthy)
        .map(|t| hex_pair(&t.n_hex, &t.e_hex))
        .collect();
    assert_eq!(healthy.len(), 6);
    let state = AppState::new();
    let (status, v) = call(&state, "POST", "/api/audit", &healthy.join("\n"));
    assert_eq!(status, 200);
    assert_eq!(v["results"].as_array().unwrap().len(), 6);

    let (status, b) = call(&state, "POST", "/api/batch-gcd", "");
    assert_eq!(status, 200);
    assert_eq!(b["keys"], 6);
    assert_eq!(b["pairs"], 15);
    assert_eq!(b["keys_affected"], 0);
}

#[test]
fn batch_gcd_exposes_the_shared_prime_sample_and_updates_reports() {
    let state = AppState::new();
    let all = samples(&state);
    let (status, v) = call(&state, "POST", "/api/audit", &sample_keys(&all, "shared-prime"));
    assert_eq!(status, 200);
    let results = v["results"].as_array().unwrap();
    assert_eq!(results.len(), 2);
    for r in results {
        // alone, each key looks clean: only the corpus view can see the shared prime
        assert_eq!(r["report"]["classification"], "NO_DEFECT_FOUND");
    }

    let (status, b) = call(&state, "POST", "/api/batch-gcd", "");
    assert_eq!(status, 200);
    assert_eq!(b["keys_affected"], 2);
    for r in results {
        let id = r["id"].as_u64().unwrap();
        let (status, report) = call(&state, "GET", &format!("/api/report/{id}"), "");
        assert_eq!(status, 200);
        assert_eq!(report["classification"], "FACTOR_RECOVERED");
        assert!(report["failure_modes"].as_array().unwrap().iter().any(|m| m == "SharedPrime"));
    }

    // re-running does not duplicate the batch signal; clearing empties the store
    call(&state, "POST", "/api/batch-gcd", "");
    let (_, report) = call(&state, "GET", &format!("/api/report/{}", results[0]["id"]), "");
    let batch_signals = report["signals"].as_array().unwrap().iter().filter(|s| s["detector"] == "batch_gcd").count();
    assert_eq!(batch_signals, 1);
    let (_, cleared) = call(&state, "DELETE", "/api/keys", "");
    assert_eq!(cleared["cleared"], 2);
    assert_eq!(call(&state, "POST", "/api/batch-gcd", "").0, 400);
}

#[test]
fn research_bundle_reports_perfect_detection_and_no_secure_labels() {
    let state = AppState::new();
    let (status, v) = call(&state, "POST", "/api/bundle", r#"{"seed": 1, "quick": true}"#);
    assert_eq!(status, 200, "{v}");
    assert_eq!(v["metrics"]["precision"], 1.0);
    assert_eq!(v["metrics"]["recall"], 1.0);
    assert_eq!(v["metrics"]["secure_labels_emitted"], 0);
    assert!(!v["ecm_study"]["rows"].as_array().unwrap().is_empty());

    assert_eq!(call(&state, "POST", "/api/bundle", "not json").0, 400);
    assert_eq!(call(&state, "POST", "/api/bundle", r#"{"bits": 64}"#).0, 400);
}

/// Send one raw HTTP request and return the full raw response.
fn raw(addr: std::net::SocketAddr, request: &str) -> String {
    let mut s = TcpStream::connect(addr).unwrap();
    s.write_all(request.as_bytes()).unwrap();
    let mut out = String::new();
    s.read_to_string(&mut out).unwrap();
    out
}

#[test]
fn http_round_trip_over_a_real_socket() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    thread::spawn(move || server::run(listener, Arc::new(AppState::new())));

    let health = raw(addr, "GET /health HTTP/1.1\r\nHost: localhost\r\n\r\n");
    assert!(health.starts_with("HTTP/1.1 200 OK"), "{health}");
    assert!(health.contains("Content-Type: application/json"));
    assert!(health.contains("\"status\": \"ok\""));

    let body = "not a key";
    let bad = raw(addr, &format!("POST /api/audit HTTP/1.1\r\nHost: localhost\r\nContent-Length: {}\r\n\r\n{body}", body.len()));
    assert!(bad.starts_with("HTTP/1.1 400"), "{bad}");

    let page = raw(addr, "GET /dashboard HTTP/1.1\r\nHost: localhost\r\n\r\n");
    assert!(page.starts_with("HTTP/1.1 200 OK") && page.contains("text/html") && page.contains("Audit a Key"));

    let huge = raw(addr, "POST /api/audit HTTP/1.1\r\nHost: localhost\r\nContent-Length: 99999999\r\n\r\n");
    assert!(huge.starts_with("HTTP/1.1 413"), "{huge}");
}
