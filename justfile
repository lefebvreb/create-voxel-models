# Checks formatting and lints the codebase.
check:
    cargo +nightly fmt --check
    cargo +stable clippy

# Formats the codebase.
fmt:
    cargo +nightly fmt

# Regenerates the `voxel.pyi` stubs file.
stubs:
    maturin generate-stubs --out .

# Run all test scripts
test:
    maturin develop
    @for f in `ls tests`; do echo -e "\033[1mpython $f\033[0m" && .venv/bin/python tests/$f; done
