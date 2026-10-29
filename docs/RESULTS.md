# RSAShield — Measured Results

Reproducible evidence for the final-year report. All numbers come from the committed code
on seeded synthetic data; regenerate with the commands shown. Hardware affects runtime but
not detection figures.

## Benchmark (detection quality)

```
cargo run --release -- bench --seed 1 --bits 512 --out bench-out
```

Default corpus: 21 keys (15 defective, 6 healthy), 512-bit moduli.

| Metric | Value |
|---|---|
| Precision | 1.000 |
| Recall | 1.000 |
| F1 | 1.000 |
| False-positive rate | 0.000 |
| Confusion (TP/FP/FN/TN) | 15 / 0 / 0 / 6 |
| `secure` labels emitted | 0 (by design) |
| Risk-score separation | healthy ≤ 10, defective ≥ 100 (margin 90) |
| Wall time | ~8 s |

Per-class recall (each defect caught by its intended detector):

| Class | Recall | Detector |
|---|---|---|
| SharedPrime | 3/3 | batch_gcd |
| ClosePrime | 3/3 | fermat |
| SmallFactor | 3/3 | small_factor |
| Unbalanced | 3/3 | ecm |
| SmallD | 3/3 | wiener |

Ground truth (private factors + labels) is written to `bench-out/manifest.json` and is
used only by the harness; it never appears in an operator report.

## ECM marginal-yield study (RQ2)

```
cargo run --release -- study-ecm --seed 1 --bits 512
```

Detection rate (and average ms/key) by factor size × ECM budget `(B1, curves)`, 512-bit
moduli, 6 keys per cell:

| factor bits | B1=500,c=10 | B1=2000,c=10 | B1=5000,c=16 | B1=10000,c=16 |
|---|---|---|---|---|
| 18 | 1.00 (12 ms) | 1.00 (23 ms) | 1.00 (39 ms) | 1.00 (85 ms) |
| 22 | 1.00 (16 ms) | 1.00 (37 ms) | 1.00 (56 ms) | 1.00 (97 ms) |
| 26 | 1.00 (15 ms) | 1.00 (50 ms) | 1.00 (154 ms) | 1.00 (385 ms) |
| 30 | **0.67** (56 ms) | 1.00 (114 ms) | 1.00 (232 ms) | 1.00 (444 ms) |

**Findings.** Detection rises with the budget and falls as the target factor grows; cost
rises with both `B1` and factor size. A 30-bit factor needs `B1 ≥ 2000`; a factor near
balanced size is out of reach at any practical budget — which is exactly why shared/close
primes are left to batch GCD and Fermat, not ECM. This is the empirical basis for treating
ECM as a bounded detector for small/unbalanced factors rather than a general weak-key oracle.

## Engineering note

The first ECM implementation (affine Weierstrass, one modular inverse per point operation)
ran the benchmark in ~359 s. Switching to Montgomery-form curves with the Montgomery ladder
(one gcd per curve) cut it to ~8 s with identical detection — a ~43× improvement.

## Tests

```
cargo test --release
```

Integration tests cover each detector, tree-vs-pairwise batch-GCD equivalence, ECM factor
recovery and yield, the no-`secure`-label invariant, operator-report redaction, and the
presence of ICS standards references on defect reports.
