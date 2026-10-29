#!/usr/bin/env bash
# RSAShield end-to-end demo (Linux/macOS/Git-Bash).
# Builds the tool, generates a labelled corpus, audits it (incl. shared-prime batch GCD),
# shows a single-key operator report with ICS standards references, and runs the ECM study.
set -euo pipefail
cd "$(dirname "$0")/.."
export PATH="$HOME/.cargo/bin:$PATH"

echo "== Building =="
cargo build --release >/dev/null
BIN=./target/release/rsashield

echo
echo "== Benchmark (detection quality on a labelled corpus) =="
"$BIN" bench --seed 1 --bits 512 --out demo-out

echo
echo "== Audit the generated corpus (first report, incl. batch GCD) =="
"$BIN" audit-corpus demo-out/keys.txt | head -40

echo
echo "== Single weak key -> operator report with ICS standards references =="
# index 12 of the corpus is a small-factor key; the operator report redacts the factor
WEAK=$(sed -n '13p' demo-out/keys.txt)
"$BIN" audit --hex "$WEAK"

echo
echo "== ECM marginal-yield study (budget vs detection vs cost) =="
"$BIN" study-ecm --seed 1 --bits 512

echo
echo "Demo complete. Ground truth is in demo-out/manifest.json (harness only)."
