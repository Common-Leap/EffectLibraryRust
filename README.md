# EffectLibraryRust

Rust library and CLI for loading and saving Nintendo Switch VFX effect files (`.eff`, `.ptcl`). Decompiles `.eff` archives into editable JSON/text assets and re-encodes them with byte-for-byte parity against the reference C# exporter.

**Crates.io:** [`effect_library`](https://crates.io/crates/effect_library) **`1.0.1`**

## Build

```bash
cd crate
cargo build --release --bin effect_dumper
```

## CLI usage

Dump an effect archive to a folder:

```bash
./target/release/effect_dumper dump /path/to/ef_mario.eff /path/to/output
```

Or install the binary from crates.io:

```bash
cargo install effect_library
effect_dumper dump /path/to/ef_mario.eff /path/to/output
```

## Using as a Rust crate

Add a dependency from [crates.io](https://crates.io/crates/effect_library):

```toml
[dependencies]
effect_library = "1.0.1"
```

Or use a path/git checkout if you are hacking on this repo:

```toml
[dependencies]
effect_library = { path = "../EffectLibraryRust/crate" }
# effect_library = { git = "https://github.com/Common-Leap/EffectLibraryRust" }
```

### Load and dump an `.eff` file

```rust
use effect_library::{Dumper, NamcoEffectFile};
use std::fs;

let data = fs::read("ef_mario.eff")?;
let namco = NamcoEffectFile::load(&data)?;

// Write NamcoFile.json, Base.ptcl, emitter folders, embedded assets, etc.
Dumper::dump_namco(&namco, "output/ef_mario")?;
```

`NamcoEffectFile` exposes the parsed EFFN header, effect entries, and an optional embedded `PtclFile`.

### Work with PTCL directly

```rust
use effect_library::PtclFile;
use std::fs;

let bytes = fs::read("Base.ptcl")?;
let ptcl = PtclFile::load(&bytes)?;

// Inspect or modify in memory, then re-encode
let roundtrip = ptcl.save();
```

You can also dump a `PtclFile` that came from a loaded `.eff`:

```rust
use effect_library::{Dumper, NamcoEffectFile};

let namco = NamcoEffectFile::load(&fs::read("ef_mario.eff")?)?;
if let Some(ptcl) = &namco.ptcl_file {
    Dumper::dump_ptcl(ptcl, "output/ptcl_only")?;
}
```

### Export embedded BFRES / BNTX / BNSH assets

Primitive models and textures live inside the PTCL blob. The submodules expose load/save helpers:

```rust
use effect_library::bfres::{export_single_model, ResFile};
use effect_library::bntx;
use effect_library::bnsh;

// Export one embedded model by descriptor-table index
let source = namco.ptcl_file.as_ref().unwrap()
    .primitive_info.as_ref().unwrap()
    .binary_data.as_ref().unwrap();
let bfres_bytes = export_single_model(source, model_index)?;

// Round-trip / normalize a standalone BFRES file
let normalized = ResFile::canonicalize(&bfres_bytes)?;

// Re-order embedded BNTX textures and re-save
let reordered = bntx::reorder_and_save(&bntx_bytes, &texture_names)?;

// Normalize BNSH shader binaries
let bnsh_bytes = bnsh::canonicalize(&shader_bytes)?;
```

### JSON metadata

`NamcoEffectFile::export_to_json()` returns the same structure written to `NamcoFile.json` during a dump, which you can serialize with `serde_json` if you want metadata without writing files to disk.

## Verification

Comparison scripts under `crate/scripts/` download and build reference tools automatically (crates.io `effect_library` **1.0.0** as a regression baseline, Joob's C# fork, and the local **1.0.1** build). Game `.eff` files cannot be downloaded automatically; the setup step symlinks `References/effect/` from a known local export path or from `$EFFECT_REFERENCE_PATH` when set.

```bash
cd crate
python3 scripts/compare_setup.py --all   # optional: prefetch everything
python3 scripts/batch_eff_compare.py     # C# vs 1.0.1 (accuracy)
python3 scripts/compare_published_vs_optimized.py  # 1.0.0 vs 1.0.1
python3 scripts/speedtest_csharp_rust.py
```

Each script calls setup on startup, compares every `.eff` under `References/effect/`, writes temp output to `References/tmp/`, and deletes it when finished.

## Credits

- [EffectLibrary](https://github.com/KillzXGaming/EffectLibrary) — reference implementation this port is based on
- [eff_lib](https://github.com/ultimate-research/eff_lib/tree/main) — original project

## Publishing (maintainers)

From the repo root, after logging in with `cargo login`:

```bash
cargo publish -p effect_library
```
