# Checks formatting and lints the codebase.
check:
    cargo +nightly fmt --check
    cargo +stable clippy

# Formats the codebase.
fmt:
    cargo +nightly fmt
