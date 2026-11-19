# Everything that must be green before a commit: format, lints, tests (guards included).
$ErrorActionPreference = 'Stop'
Set-Location (Split-Path $PSScriptRoot -Parent)
cargo fmt --all -- --check
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
cargo clippy --workspace --all-targets -- -D warnings
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
# Production code (not tests or examples) never unwraps: a failure is an error or a log.
cargo clippy --workspace --lib --bins -- -D clippy::unwrap_used
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
cargo test --workspace
exit $LASTEXITCODE
