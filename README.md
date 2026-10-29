# RSAShield

An **offline, multi-signal RSA public-key quality auditor** for authorised security
reviews of long-lived industrial/ICS equipment (e.g. substation devices). It flags RSA
keys likely produced by broken or entropy-starved key generation so operators can
prioritise replacement.

> **Academic / defensive project.** RSAShield analyses *public* keys only, runs fully
> offline, and is developed and tested exclusively against synthetic and explicitly
> authorised data. It never probes, scans, or authenticates to real equipment. See
> [`docs/THREAT_MODEL.md`](docs/THREAT_MODEL.md).

## The idea in one paragraph

Weak entropy during key generation produces a few distinct, detectable defects, and no
single algorithm catches all of them. RSAShield runs **complementary detectors** and
fuses their results:

| Detector | Catches | Failure mode |
|---|---|---|
| `metadata` | undersized moduli, bad exponents, skewed modulus statistics | advisory |
| `small_factor` | tiny factors via trial division | small/unbalanced factor |
| `fermat` | primes chosen too close together | close primes |
| `wiener` | dangerously small private exponent `d` | small `d` |
| `ecm` | medium/unbalanced factors beyond trial division (bounded) | small/unbalanced factor |
| `batch_gcd` | a prime shared between two keys in the corpus | shared prime |

**On ECM.** Lenstra's Elliptic Curve Method is a **bounded detector** for the
**small/unbalanced-factor** case only — its cost depends on the smallest factor, not on
`n`, so it cannot factor a balanced weak-entropy modulus. It runs under a strict
`(B1, curves)` budget (Montgomery form, one gcd per curve), and a failed ECM run proves
nothing. Entropy failures are caught mainly by **batch GCD** (shared primes) and
**Fermat** (close primes). This correction is central to the project; see
[`docs/RESEARCH_PLAN.md`](docs/RESEARCH_PLAN.md).

**No "secure" verdict.** The best result RSAShield reports is `NO_DEFECT_FOUND`, meaning
only that no detector fired under its budget — never a proof of security. Recovered
private factors never appear in operator reports; they stay in the forensic/benchmark
layer.

## Build & run

```sh
cargo build --release

# audit one key (PEM/DER file, or - for stdin)
cargo run --release -- audit path/to/key.pem

# audit a hex modulus:exponent pair
cargo run --release -- audit --hex "<modulus_hex>:10001"

# audit many keys together (enables shared-prime batch GCD)
cargo run --release -- audit-corpus keys.txt

# run the reproducible labelled benchmark (precision/recall/runtime)
cargo run --release -- bench --seed 1 --bits 512 --out bench-out

# ECM marginal-yield study: detection vs budget vs cost (research question RQ2)
cargo run --release -- study-ecm --seed 1 --bits 512
```

`bench` generates a synthetic corpus with known defects and ground-truth factors,
evaluates every detector against the labels, and reports precision, recall, per-class
detection, false-positive rate, and runtime. Ground truth is written to
`bench-out/manifest.json` (harness only).

## Web interface

```sh
cargo run --release -- serve            # http://127.0.0.1:8080
cargo run --release -- serve --addr 127.0.0.1:9000
```

`serve` starts a local, dependency-free web server (std `TcpListener`, loopback only by
default) with three pages:

- **`/`** — project overview.
- **`/dashboard`** — the audit console: paste PEM or hex `n:e` keys (one or many), or pick
  a seeded sample per defect class; read the operator report (classification, risk score,
  failure modes, remediation, ICS standards, per-detector signals); run batch GCD across
  every key audited this session; and run the **research bundle** (benchmark + ECM
  marginal-yield study) with its metrics, per-class recall and ECM heatmap.
- **`/related-work`** — prior work and the gaps this project addresses.

The same data is available as JSON:

| Method & path | Purpose |
|---|---|
| `GET /health` | liveness probe |
| `GET /api/samples` | seeded demo keys, one per defect class (plus a malformed PEM) |
| `POST /api/audit` | audit the key(s) in the body and store them; `400` with a parse error on bad input |
| `GET /api/keys` / `DELETE /api/keys` | list / clear the session's stored keys |
| `GET /api/report/{id}` | operator report for one stored key |
| `POST /api/batch-gcd` | tree batch GCD across all stored keys; findings fold back into their reports |
| `POST /api/bundle` | research bundle; optional body `{"seed": 1, "bits": 512, "quick": false}` |

Like the CLI, the API returns operator reports only — no endpoint can carry a recovered
factor. The UI is plain HTML/CSS/JS in [`web/`](web/) (no build step), embedded into the
binary at compile time; its isometric illustrations are drawn at runtime by a small SVG kit
(`web/iso.js`, `web/scenes.js`). Opening `web/index.html` directly also works for the
overview page; the dashboard needs the server.

## Status

All six roadmap phases are implemented: the deterministic MVP; the reproducible benchmark;
Bernstein product/remainder-tree batch GCD (alongside the pairwise reference); bounded
Montgomery-form ECM with an `unbalanced` class only ECM catches; the ECM marginal-yield
study (RQ2) and risk-score separation; and the ICS/substation layer — IEC 62351 / NERC CIP /
NIST references attached to each defect report, remediation context, and a demo. The
deferred UI pass is also complete: `rsashield serve` provides the web dashboard and JSON
API described above, covering the original prototype's feature surface (health,
dashboard, related work, per-detector samples, bad-PEM `400`, batch GCD over stored keys,
report JSON, research bundle).

On the default corpus the benchmark reports precision/recall/F1 = 1.000, zero false
positives, zero "secure" labels, and clean risk-score separation (healthy ≤ 10,
defective ≥ 100). Measured results: [`docs/RESULTS.md`](docs/RESULTS.md). ICS remediation
mapping: [`docs/ICS_REMEDIATION.md`](docs/ICS_REMEDIATION.md). Run the walkthrough with
`scripts/demo.ps1` (Windows) or `scripts/demo.sh`. Tests: `cargo test --release` (detector
integration tests in `tests/integration.rs`, web API tests in `tests/server.rs`). Roadmap:
[`docs/RESEARCH_PLAN.md`](docs/RESEARCH_PLAN.md).

## License

MIT OR Apache-2.0.
