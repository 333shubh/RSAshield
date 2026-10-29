# Build the in-browser engine (web/rsashield.wasm) used by the hosted website.
# Needs the wasm32 target once:  rustup target add wasm32-unknown-unknown

$ErrorActionPreference = "Stop"
Set-Location (Split-Path $PSScriptRoot -Parent)
$env:Path = "$env:USERPROFILE\.cargo\bin;" + $env:Path

cargo rustc --lib --release --target wasm32-unknown-unknown --crate-type cdylib
if ($LASTEXITCODE -ne 0) { throw "wasm build failed" }
Copy-Item target\wasm32-unknown-unknown\release\rsashield.wasm web\rsashield.wasm -Force
Write-Host ("wrote web\rsashield.wasm ({0} bytes)" -f (Get-Item web\rsashield.wasm).Length)
