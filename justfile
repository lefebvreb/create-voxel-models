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
    # Create target directories.
    mkdir -p create-voxel-models/dist
    mkdir -p create-voxel-models/references
    # Clear previous artifacts.
    rm -f create-voxel-models.zip
    rm -f create-voxel-models/references/*
    rm -f create-voxel-models/dist/*
    # Build manylinux package wheel file and place it in the dist directory.
    maturin build --release --strip --zig --out create-voxel-models/dist
    # Copy pyi stubs to the references directory.
    cp voxels.pyi create-voxel-models/references
    # Update SKILL.md front matter.
    yq -i --front-matter=process \
        ".author = \"$(yq -p toml '.package.authors[0]' Cargo.toml)\" \
        | .version = \"$(yq -p toml '.package.version' Cargo.toml)\" \
        | .tags = $(yq -p toml -o=json -I=0 '.package.keywords' Cargo.toml) | (.tags style=\"flow\")" \
        create-voxel-models/SKILL.md
    # Zip skill into an archive.
    zip -r create-voxel-models.zip create-voxel-models
