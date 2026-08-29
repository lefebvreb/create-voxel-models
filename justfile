# Checks formatting, lints the codebase, and checks that `.pyi` stubs are up-to-date.
check:
    cargo +nightly fmt --check
    cargo +stable clippy -- -Dwarnings
    maturin generate-stubs --out /tmp -- -Awarnings && diff python/voxels/_voxels.pyi /tmp/_voxels.pyi

# Formats the codebase.
fmt:
    cargo +nightly fmt

# Lints the codebase.
clippy:
    cargo +stable clippy

# Regenerates the pyi stubs file.
stubs:
    maturin generate-stubs --out python/voxels -- -Awarnings

# Compiles the project in debug mode, bundles it as a python package and runs all test scripts in a venv.
test:
    cargo +stable test
    maturin develop -- -Awarnings
    @for file in `ls tests/*.py`; do echo -e "\033[1mpython $file\033[0m" && .venv/bin/python $file; done

# Builds the project in release mode, bundles all files into a zipped agent skill.
build-skill: check
    # Remove previous wheel artifacts.
    rm -f target/wheels/*
    # Build manylinux package wheel file.
    maturin build --release --strip --zig
    # Update SKILL.md front matter.
    yq -i --front-matter=process \
        " .description = \"$(yq -p toml -o yaml '.package.description' Cargo.toml)\" \
        | .metadata.author = \"$(yq -p toml -o yaml '.package.authors[0]' Cargo.toml)\" \
        | .metadata.version = \"$(yq -p toml -o yaml '.package.version' Cargo.toml)\"" \
        SKILL.md
    # Zip SKILL.md, wheel and pyi stubs into an archive.
    python -c "from pathlib import Path; \
        from zipfile import ZipFile; \
        wheel = next(Path('target/wheels/').iterdir()); \
        f = ZipFile('create-voxel-models.zip', 'w'); \
        f.write('SKILL.md'); \
        f.write('LICENSE'); \
        f.write(wheel, arcname=f'dist/{wheel.name}'); \
        f.write('python/voxels/_voxels.pyi', arcname='references/voxels.pyi');"
