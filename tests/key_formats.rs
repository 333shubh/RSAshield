//! Real-world input formats (made with openssl / ssh-keygen; public material only),
//! private-key refusal, and the browser build's detector budget.

use rsashield::api::{self, AppState};
use rsashield::key;
use serde_json::Value;

const CERT: &str = include_str!("fixtures/rsa2048-cert.pem");
const SPKI: &str = include_str!("fixtures/rsa2048-spki.pem");
const SSH: &str = include_str!("fixtures/rsa2048.pub");
const MODULUS: &str = include_str!("fixtures/rsa2048-modulus.hex");
const EC_CERT: &str = include_str!("fixtures/ec-p256-cert.pem");

fn call(state: &AppState, method: &str, path: &str, body: &str) -> (u16, Value) {
    let r = api::handle(state, method, path, body.as_bytes());
    (r.status, serde_json::from_slice(&r.body).unwrap_or(Value::Null))
}

#[test]
fn certificate_spki_and_ssh_rsa_yield_the_same_key() {
    let expected = MODULUS.trim();
    for (name, input) in [("certificate", CERT), ("spki", SPKI), ("ssh-rsa", SSH)] {
        let k = key::parse_any(input, name).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(k.n.to_str_radix(16), expected, "{name} modulus");
        assert_eq!(k.e, 65537u32.into(), "{name} exponent");
        assert_eq!(k.modulus_bits(), 2048, "{name} size");
    }
}

#[test]
fn non_rsa_keys_are_named_not_misparsed() {
    let e = key::parse_any(EC_CERT, "ec").unwrap_err().to_string();
    assert!(e.contains("elliptic-curve"), "{e}");
    let e = key::parse_any("ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIGs= x@y", "ed").unwrap_err().to_string();
    assert!(e.contains("not an RSA key"), "{e}");
}

#[test]
fn a_list_can_mix_formats() {
    let mixed = format!("{CERT}\n{SSH}\n{}:10001\n", MODULUS.trim());
    let records = key::parse_many(&mixed, "mixed");
    assert_eq!(records.len(), 3);
    assert!(records.iter().all(|r| r.is_ok()));
}

#[test]
fn private_keys_are_refused_before_anything_is_stored() {
    let state = AppState::browser();
    for pasted in [
        "-----BEGIN PRIVATE KEY-----\nMIIEvQIBADANBgkqhkiG9w0BAQEFAASC\n-----END PRIVATE KEY-----",
        "-----BEGIN RSA PRIVATE KEY-----\nMIIEowIBAAKCAQEA\n-----END RSA PRIVATE KEY-----",
        "-----BEGIN OPENSSH PRIVATE KEY-----\nb3BlbnNzaC1rZXktdjEAAAAA\n-----END OPENSSH PRIVATE KEY-----",
        "PuTTY-User-Key-File-3: ssh-rsa\nEncryption: none",
    ] {
        assert!(key::parse_any(pasted, "x").unwrap_err().to_string().contains("PRIVATE key"));
        let (status, v) = call(&state, "POST", "/api/audit", pasted);
        assert_eq!(status, 400);
        assert_eq!(v["private_key"], true);
    }
    let (_, keys) = call(&state, "GET", "/api/keys", "");
    assert_eq!(keys["keys"].as_array().unwrap().len(), 0, "nothing may be stored");
}

#[test]
fn browser_budget_still_catches_every_sample() {
    let state = AppState::browser();
    let (_, health) = call(&state, "GET", "/health", "");
    assert_eq!(health["mode"], "browser");
    assert_eq!(health["budget"], "standard");

    let (_, samples) = call(&state, "GET", "/api/samples", "");
    let keys_of = |id: &str| -> String {
        let s = samples.as_array().unwrap().iter().find(|s| s["id"] == id).unwrap();
        s["keys"].as_array().unwrap().iter().map(|k| k.as_str().unwrap()).collect::<Vec<_>>().join("\n")
    };
    for (id, detector) in [("small-factor", "small_factor"), ("close-primes", "fermat"), ("small-d", "wiener"), ("unbalanced", "ecm")] {
        let (status, v) = call(&state, "POST", "/api/audit", &keys_of(id));
        assert_eq!(status, 200, "{id}");
        assert_eq!(v["budget"], "standard");
        let report = &v["results"][0]["report"];
        assert_eq!(report["classification"], "FACTOR_RECOVERED", "{id}");
        assert!(report["signals"].as_array().unwrap().iter().any(|s| s["detector"] == detector && s["verdict"] == "FactorRecovered"), "{id} via {detector}");
    }
}

#[test]
fn a_real_2048_bit_certificate_audits_cleanly_and_budget_is_validated() {
    let state = AppState::browser();
    let (status, v) = call(&state, "POST", "/api/audit", CERT);
    assert_eq!(status, 200);
    let report = &v["results"][0]["report"];
    assert_eq!(report["classification"], "NO_DEFECT_FOUND");
    assert_eq!(report["modulus_bits"], 2048);
    assert!(report["standards"].as_array().unwrap().is_empty(), "a sound 2048-bit key needs no standards references");

    assert_eq!(call(&state, "POST", "/api/audit?budget=extreme", SSH).0, 400);
}

#[test]
fn reauditing_a_key_replaces_it_instead_of_faking_a_duplicate() {
    let state = AppState::browser();
    let (_, first) = call(&state, "POST", "/api/audit", CERT);
    // same key again, as an ssh-rsa line, with the deep budget
    let (status, again) = call(&state, "POST", "/api/audit?budget=deep", SSH);
    assert_eq!(status, 200);
    assert_eq!(again["results"][0]["id"], first["results"][0]["id"]);
    assert_eq!(again["results"][0]["reaudited"], true);
    let (_, keys) = call(&state, "GET", "/api/keys", "");
    assert_eq!(keys["keys"].as_array().unwrap().len(), 1);
    // with one stored key there is nothing to compare, so no false "shared prime"
    assert_eq!(call(&state, "POST", "/api/batch-gcd", "").0, 400);

    // but the same key twice in one submitted list is a genuine duplicate finding
    let fleet = AppState::browser();
    call(&fleet, "POST", "/api/audit", &format!("{SSH}\n{SSH}"));
    let (_, b) = call(&fleet, "POST", "/api/batch-gcd", "");
    assert_eq!(b["keys_affected"], 2);
}
