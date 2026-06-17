# EffectLibraryRust

Rust library and CLI for loading and saving Nintendo Switch VFX effect files (`.eff`, `.ptcl`). Decompiles `.eff` archives into editable JSON/text assets and re-encodes them with byte-for-byte parity against the reference C# exporter.

## Build

```bash
cd crate
cargo build --release --bin effect_dumper
```

## Usage

Dump an effect archive to a folder:

```bash
./target/release/effect_dumper dump /path/to/ef_mario.eff /path/to/output
```

## Verification

Place game `.eff` files and a built [EffectLibrary](https://github.com/KillzXGaming/EffectLibrary) `EffectConverter` under `References/` (not committed). Then run the batch comparison script:

```bash
cd crate
cargo build --release --bin effect_dumper
python3 scripts/batch_eff_compare.py
```

The script compares Rust output against C# for every `.eff` under `References/effect/` and cleans up temp dirs when finished.

## Credits

- [EffectLibrary](https://github.com/KillzXGaming/EffectLibrary) — reference implementation this port is based on
- [eff_lib](https://github.com/ultimate-research/eff_lib/tree/main) — original project
