# EffectLibraryRust

Rust library and CLI for loading and saving Nintendo Switch VFX effect files (`.eff`, `.ptcl`). Decompiles `.eff` archives into editable JSON/text assets and re-encodes them with byte-for-byte parity against the reference C# exporter.

**Crates.io:** [`effect_library`](https://crates.io/crates/effect_library) **`1.1.0`**

## Build

From the repo root:

```bash
cargo build --release --bin effect_converter
```

The binary is written to `target/release/effect_converter`.

## CLI usage

Decompile an effect archive to a folder:

```bash
./target/release/effect_converter dump /path/to/ef_mario.eff /path/to/output
```

Recompile a decompiled folder back to `.eff`:

```bash
./target/release/effect_converter build /path/to/output/ef_mario /path/to/ef_mario_NEW.eff
```

Or install the `effect_library` crate from crates.io (binary name: `effect_converter`):

```bash
cargo install effect_library
effect_converter dump /path/to/ef_mario.eff /path/to/output
effect_converter build /path/to/output/ef_mario /path/to/ef_mario_NEW.eff
```

Header-only effects (no `Base.ptcl`) skip build, matching C# behavior.

## Using as a Rust crate

Add a dependency from [crates.io](https://crates.io/crates/effect_library):

```toml
[dependencies]
effect_library = "1.1.0"
```

Or use a path/git checkout when working on this repo:

```toml
[dependencies]
effect_library = { path = "crate" }
# effect_library = { git = "https://github.com/Common-Leap/EffectLibraryRust" }
```

### Load and dump an `.eff` file

```rust
use effect_library::{Dumper, NamcoEffectFile};
use std::fs;

let data = fs::read("ef_mario.eff")?;
let namco = NamcoEffectFile::load(&data)?;

Dumper::dump_namco(&namco, "output/ef_mario")?;
```

### Rebuild from a decompiled folder

```rust
use effect_library::Creator;
use std::fs;

let namco = Creator::create_namco_from_folder("output/ef_mario")?
    .expect("effect has Base.ptcl");
fs::write("ef_mario_NEW.eff", namco.save()?)?;
```

`Creator::create_ptcl_from_folder` rebuilds only the embedded PTCL when you do not need the EFFN wrapper.

### Work with PTCL directly

```rust
use effect_library::PtclFile;
use std::fs;

let bytes = fs::read("Base.ptcl")?;
let ptcl = PtclFile::load(&bytes)?;
let roundtrip = ptcl.save();
```

### Export embedded BFRES / BNTX / BNSH assets

```rust
use effect_library::bfres::{export_single_model, ResFile};
use effect_library::bntx;
use effect_library::bnsh;

let source = namco.ptcl_file.as_ref().unwrap()
    .primitive_info.as_ref().unwrap()
    .binary_data.as_ref().unwrap();
let bfres_bytes = export_single_model(source, model_index)?;
let normalized = ResFile::canonicalize(&bfres_bytes)?;
let reordered = bntx::reorder_and_save(&bntx_bytes, &texture_names)?;
let bnsh_bytes = bnsh::canonicalize(&shader_bytes)?;
```

## Verification

Scripts under `crate/scripts/` download/build reference tools and locate local game `.eff` files automatically. Game assets are not redistributable; setup symlinks `References/effect/` from a known export path or `$EFFECT_REFERENCE_PATH`.

```bash
cd crate
python3 scripts/compare_setup.py --all          # optional: prefetch tools + effect symlink
python3 scripts/batch_eff_roundtrip.py          # C# dump → C# vs Rust rebuild (328 effects)
python3 scripts/batch_eff_compare.py            # C# vs local dump accuracy
python3 scripts/compare_published_vs_optimized.py  # crates.io 1.0.0 vs local
python3 scripts/speedtest_csharp_rust.py        # dump-only timing
python3 scripts/speedtest_roundtrip.py --csharp # dump + build timing
```

Temp output goes under `References/tmp/` and is removed when each script finishes.

Integration tests in `creator.rs` optionally use folders under `/tmp/` when present; they skip otherwise.

## Credits

- [EffectLibrary](https://github.com/KillzXGaming/EffectLibrary) — reference implementation this port is based on
- [Joob's EffectLibrary fork](https://github.com/joobert/EffectLibrary) — C# EffectConverter used for parity testing (and some crash fixes from the original C# version)
- [eff_lib](https://github.com/ultimate-research/eff_lib/tree/main) — original project

## Publishing (maintainers)

From the repo root, after `cargo login`:

```bash
cargo publish -p effect_library
```
