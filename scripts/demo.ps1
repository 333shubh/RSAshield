# RSAShield end-to-end demo (Windows / PowerShell).
# Builds the tool, generates a labelled corpus, audits it (incl. shared-prime batch GCD),
# shows a single-key operator report with ICS standards references, and runs the ECM study.

$ErrorActionPreference = "Stop"
Set-Location (Split-Path $PSScriptRoot -Parent)
$env:Path = "$env:USERPROFILE\.cargo\bin;" + $env:Path

Write-Host "== Building ==" -ForegroundColor Cyan
cargo build --release | Out-Null
$bin = ".\target\release\rsashield.exe"

Write-Host "`n== Benchmark (detection quality on a labelled corpus) ==" -ForegroundColor Cyan
& $bin bench --seed 1 --bits 512 --out demo-out

Write-Host "`n== Audit the generated corpus (first report, incl. batch GCD) ==" -ForegroundColor Cyan
& $bin audit-corpus demo-out/keys.txt | Select-Object -First 40

Write-Host "`n== Single weak key -> operator report with ICS standards references ==" -ForegroundColor Cyan
# index 12 of the corpus is a small-factor key; the operator report redacts the factor
$weak = (Get-Content demo-out/keys.txt)[12]
& $bin audit --hex $weak

Write-Host "`n== ECM marginal-yield study (budget vs detection vs cost) ==" -ForegroundColor Cyan
& $bin study-ecm --seed 1 --bits 512

Write-Host "`nDemo complete. Ground truth is in demo-out/manifest.json (harness only)." -ForegroundColor Green
