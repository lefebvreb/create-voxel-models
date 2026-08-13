# Checks formatting, lints the codebase, and checks that `.pyi` stubs are up-to-date.
check:
    cargo +nightly fmt --check
    cargo +stable clippy -- -Dwarnings
    maturin generate-stubs --out /tmp && diff voxels.pyi /tmp/voxels.pyi

# Formats the codebase.
fmt:
    cargo +nightly fmt

# Lints the codebase.
clippy:
    cargo +stable clippy

# Regenerates the `.pyi` stubs file.
stubs:
    maturin generate-stubs --out .

# Compiles the project in debug mode, bundles it as a python module and runs all test scripts in a venv.
test:
    maturin develop
    @for f in `ls tests`; do echo -e "\033[1mpython $f\033[0m" && .venv/bin/python tests/$f; done
