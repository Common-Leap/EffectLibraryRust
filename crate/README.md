# effect_library

Rust library and CLI for Nintendo Switch VFX effect files (`.eff`, `.ptcl`). Decompiles archives into editable JSON/text assets and re-encodes them with byte-for-byte parity against the reference C# exporter.

**Crates.io:** [`effect_library`](https://crates.io/crates/effect_library) **`1.1.0`**

## Build

From the repo root:

```bash
cargo build --release --bin effect_converter
```

Or from this directory:

```bash
cargo build --release --bin effect_converter
```

## CLI usage

```bash
effect_converter dump /path/to/ef_mario.eff /path/to/output
effect_converter build /path/to/output/ef_mario /path/to/ef_mario_NEW.eff
```

Install the crate from crates.io (binary name: `effect_converter`):

```bash
cargo install effect_library
```

## Library API

```toml
[dependencies]
effect_library = "1.1.0"
```

```rust
use effect_library::{Creator, Dumper, NamcoEffectFile, PtclFile};
use std::fs;

// Load and dump
let namco = NamcoEffectFile::load(&fs::read("ef_mario.eff")?)?;
Dumper::dump_namco(&namco, "output/ef_mario")?;

// Rebuild
let rebuilt = Creator::create_namco_from_folder("output/ef_mario")?
    .expect("effect has Base.ptcl");
fs::write("ef_mario_NEW.eff", rebuilt.save()?)?;

// PTCL only
let ptcl = PtclFile::load(&fs::read("Base.ptcl")?)?;
let bytes = ptcl.save();
```

Submodules `bfres`, `bntx`, and `bnsh` expose load/save helpers for embedded asset pools.

## Verification

See the [repository README](../README.md#verification) for comparison scripts. Run from this directory:

```bash
python3 scripts/batch_eff_roundtrip.py
python3 scripts/speedtest_roundtrip.py --csharp
```

Requires local `.eff` files (see `scripts/compare_setup.py`).

## Credits

- [EffectLibrary](https://github.com/KillzXGaming/EffectLibrary)
- [Joob's EffectLibrary fork](https://github.com/joobert/EffectLibrary)
- [eff_lib](https://github.com/ultimate-research/eff_lib/tree/main)
