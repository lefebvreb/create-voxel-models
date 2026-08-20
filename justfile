# Checks formatting, lints the codebase, and checks that `.pyi` stubs are up-to-date.
check:
    cargo +nightly fmt --check
    cargo +stable clippy -- -Dwarnings
    maturin generate-stubs --out /tmp -- -Awarnings && diff voxels.pyi /tmp/voxels.pyi

# Formats the codebase.
fmt:
    cargo +nightly fmt

# Lints the codebase.
clippy:
    cargo +stable clippy

# Regenerates the `.pyi` stubs file.
stubs:
    maturin generate-stubs --out . -- -Awarnings

# Compiles the project in debug mode, bundles it as a python module and runs all test scripts in a venv.
test:
    cargo +stable test
    maturin develop -- -Awarnings
    @for f in `ls tests/*.py`; do echo -e "\033[1mpython $f\033[0m" && .venv/bin/python $f; done

# Builds the project in release mode.
build:
    rm -rf create-voxel-models/dist
    rm -rf create-voxel-models/references
    maturin build --release --strip --zig --out create-voxel-models/dist
    mkdir -p create-voxel-models/references
    cp voxels.pyi create-voxel-models/references
    zip -r create-voxel-models.zip create-voxel-models
