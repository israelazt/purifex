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
# What CI builds on Linux: the portable layers must not need Windows (a module of the platform
# that uses `windows` has to be behind #[cfg(windows)]).
cargo clippy --target x86_64-unknown-linux-gnu -p purifex-domain -p purifex-app -p purifex-platform -p purifex-ui --all-targets -- -D warnings
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
cargo test --workspace
exit $LASTEXITCODE
