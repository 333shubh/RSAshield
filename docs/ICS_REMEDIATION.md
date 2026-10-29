# ICS / Substation Remediation Context

This document connects RSAShield's findings to the operational reality of industrial
control systems (ICS) and electrical substations, and to the security standards those
environments work under. RSAShield emits the matching references in each operator report
(the `standards` field).

> **Indicative, not compliance.** These are triage references to help route a finding into
> existing assurance processes. RSAShield analyses public-key material only and cannot make
> a compliance determination. See [`THREAT_MODEL.md`](THREAT_MODEL.md).

## Why weak keys matter more in substations

- **Keys live for years.** Substation RTUs, IEDs, and gateways are provisioned once and may
  run a decade or more. A weak key generated at manufacture persists far longer than in IT.
- **Rotation is expensive.** Replacing a key can mean a maintenance window, a site visit, or
  vendor involvement — so prioritisation matters, which is what the risk score is for.
- **Entropy is scarce at provisioning.** Headless devices generating keys at first boot are
  exactly the conditions that produce shared and close primes (the batch-GCD and Fermat
  failure modes), and the defect is usually shared across a whole batch or firmware build.
- **Fleet correlation.** A single shared prime implicates every sibling device — so a
  positive batch-GCD result is a signal to audit the batch, not just the one unit.

## Failure mode → operational impact → references

| Failure mode | What it means operationally | Standards references |
|---|---|---|
| Shared prime | Weak entropy at provisioning; likely whole-batch | IEC 62351-9 (RNG at key generation), NERC CIP-010 R3 |
| Close primes | Defective prime generation in the device library | IEC 62351-9 (prime generation), NERC CIP-010 R3 |
| Small / unbalanced factor | Broken key-generation implementation | IEC 62351-9 (key-generation integrity), NERC CIP-010 R3 |
| Small private exponent | Non-standard key parameters for speed | IEC 62351-9 (key parameter selection), NERC CIP-010 R3 |
| Structured prime | Known library/firmware flaw (e.g. ROCA-style) | Vendor advisory, IEC 62351-9 |
| Weak metadata (size/exponent) | Below current key-length/algorithm policy | NIST SP 800-57 Pt 1, IEC 62351-3 (TLS profile) |

## Standards referenced

- **IEC 62351-9** — Key management for power-system equipment: generation, distribution,
  and lifecycle, including revocation and reissue of compromised keys. The primary reference
  for any recovered or suspected weak key.
- **IEC 62351-3** — Security for power-system TCP/IP profiles (TLS): algorithm and key-length
  requirements for substation communications.
- **NERC CIP-010** — Configuration change management and vulnerability assessments: a weak
  key is a vulnerability to record and track to remediation (R3).
- **NIST SP 800-57 Part 1** — key-size and algorithm-lifetime recommendations, for the
  undersized-modulus advisory.

## Remediation priority (how to read a report)

1. `FACTOR_RECOVERED` (risk 100): the key is broken. Revoke and replace; audit the batch.
2. `DEFECT_SUSPECTED`: quarantine and confirm against the device library/firmware version.
3. `NO_DEFECT_FOUND`: no action forced — but **not** a proof of security. Re-audit with a
   wider ECM budget and against a larger corpus when feasible.
