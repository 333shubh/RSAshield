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
| `batch_gcd` | a prime shared between two keys in the corpus | shared prime |

**On ECM.** Lenstra's Elliptic Curve Method is a *planned bounded detector* (Phase 4)
for the **small/unbalanced-factor** case only — its cost depends on the smallest factor,
not on `n`, so it cannot factor a balanced weak-entropy modulus. Entropy failures are
caught mainly by **batch GCD** (shared primes) and **Fermat** (close primes). A failed
ECM run proves nothing. This correction is central to the project; see
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
```

`bench` generates a synthetic corpus with known defects and ground-truth factors,
evaluates every detector against the labels, and reports precision, recall, per-class
detection, false-positive rate, and runtime. Ground truth is written to
`bench-out/manifest.json` (harness only).

## Status

Phase 1 (deterministic MVP) and Phase 2 (benchmark) are implemented. Batch GCD uses the
pairwise method (Phase 3 upgrades to Bernstein's product/remainder tree); bounded ECM is
Phase 4. See the roadmap in [`docs/RESEARCH_PLAN.md`](docs/RESEARCH_PLAN.md).

## License

MIT OR Apache-2.0.
