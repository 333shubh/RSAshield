# RSAShield — Threat Model, Scope & Ethics

RSAShield is a defensive, academic tool. This document states what it does, what it
deliberately does not do, and the rules the project operates under.

## Purpose

Given RSA **public** keys obtained with authorisation (device inventories, certificate
exports, configuration backups), identify keys likely produced by broken or
entropy-starved key generation so that operators can prioritise their replacement. The
motivating environment is long-lived industrial/ICS equipment such as electrical
substation devices, where keys are hard to rotate and may persist for a decade or more.

## Authorised-use boundaries (binding)

- **Synthetic and authorised data only.** Development and evaluation use synthetic keys
  and deliberately-generated weak fixtures. Any real key material is analysed only with
  explicit permission from its owner.
- **Offline.** The tool reads keys from files and has no network client for operational
  technology. It never connects to, probes, scans, or authenticates to any device.
- **Public material only.** RSAShield consumes `(n, e)`. It is not given, and does not
  require, any private key.
- **No exploitation.** Recovering a factor in this tool demonstrates that a key is
  already broken; the tool's output is a remediation prompt, not an attack.

## Assets and what "compromise" means

The protected asset is the confidentiality of the RSA private key, which rests entirely
on the secrecy of the factorisation `n = p·q`. A key is "compromised" here when `p` (and
hence the private key) is recoverable from public data by a modest offline effort.

## Adversary and failure modes addressed

The adversary is anyone who can read the public key and run standard offline
mathematics. The defects RSAShield screens for are key-generation failures, not protocol
or implementation attacks:

- **Shared prime** across devices (weak entropy) → `batch_gcd`.
- **Close primes** within one key → `fermat`.
- **Small / unbalanced factor** → `small_factor` (and bounded ECM, planned).
- **Small private exponent** → `wiener`.
- **Weak metadata** (undersized modulus, bad exponent, skewed statistics) → advisory.

Out of scope: side channels, padding oracles, signature-nonce reuse, key storage/HSM
protection, certificate trust/revocation, and protocol-level issues.

## Data handling and redaction

- **Operator reports never contain recovered factors.** The report type has no field
  that can carry `p` or `q`, and detector detail strings are written to describe a factor
  (e.g. its bit length) without printing its value.
- **Recovered factors and ground-truth labels live only in the forensic/benchmark
  layer.** The benchmark writes ground truth (including factors) to its manifest, which
  is harness output, not operator output.
- The modulus `n` and exponent `e` are public and may appear in reports.

## What a clean result does and does not mean

RSAShield has **no "secure" verdict**. The strongest negative result is
`NO_DEFECT_FOUND`, which means only that no detector fired under its configured budget.
It is not a proof of security: untested factoring paths (for example bounded ECM) and
shared primes outside the audited corpus may still apply.

## On ECM specifically

Lenstra's Elliptic Curve Method is a bounded detector for the small/unbalanced-factor
case only. Its cost depends on the size of the smallest factor, so it cannot factor a
balanced weak-entropy modulus. It is one signal among several, and a failed ECM attempt
is not evidence that a key is sound. See `docs/RESEARCH_PLAN.md` §1.
