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

# Regenerates the pyi stubs file.
stubs:
    maturin generate-stubs --out . -- -Awarnings

# Compiles the project in debug mode, bundles it as a python module and runs all test scripts in a venv.
test:
    cargo +stable test
    maturin develop -- -Awarnings
    @for file in `ls tests/*.py`; do echo -e "\033[1mpython $file\033[0m" && .venv/bin/python $file; done

# Builds the project in release mode.
build:
    # Remove previous wheel artifacts.
    rm -f target/wheels/*
    # Build manylinux package wheel file.
    maturin build --release --strip --zig
    # Update SKILL.md front matter.
    yq -i --front-matter=process \
        " .description = \"$(yq -p toml '.package.description' Cargo.toml)\" \
        | .author = \"$(yq -p toml '.package.authors[0]' Cargo.toml)\" \
        | .version = \"$(yq -p toml '.package.version' Cargo.toml)\" \
        | .tags = $(yq -p toml -o=json -I=0 '.package.keywords' Cargo.toml) \
        | (.tags style=\"flow\")" \
        SKILL.md
    # Zip SKILL.md, wheel and pyi stubs into an archive.
    python -c "from pathlib import Path; \
        from zipfile import ZipFile; \
        wheel = next(Path('target/wheels/').iterdir()); \
        f = ZipFile('create-voxel-models.zip', 'w'); \
        f.write('SKILL.md'); \
        f.write('LICENSE'); \
        f.write(wheel, arcname=f'dist/{wheel.name}'); \
        f.write('voxels.pyi', arcname='references/voxels.pyi');"
