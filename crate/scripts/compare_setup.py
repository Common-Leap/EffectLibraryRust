#!/usr/bin/env python3
"""Download/build reference tools and locate effect files for comparison scripts."""

from __future__ import annotations

import os
import shutil
import subprocess
import sys
import tarfile
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
REFS = ROOT / "References"
TMP = REFS / "tmp"

PUBLISHED_CRATE_VERSION = "1.0.0"
PUBLISHED_CRATE_URL = (
    f"https://static.crates.io/crates/effect_library/"
    f"effect_library-{PUBLISHED_CRATE_VERSION}.crate"
)
PUBLISHED_SRC = REFS / f"effect_library-{PUBLISHED_CRATE_VERSION}"
# crates.io 1.0.0 shipped the CLI as effect_dumper
PUBLISHED_BIN = PUBLISHED_SRC / "target" / "release" / "effect_dumper"

CS_REPO = "https://github.com/joobert/EffectLibrary.git"
CS_DIR = REFS / "EffectLibrary"
CS_BIN = CS_DIR / "EffectConverter" / "bin" / "Release" / "publish" / "EffectConverter"
CS_BIN_FALLBACK = ROOT / "bin" / "Release" / "publish" / "EffectConverter"

OPTIMIZED_BIN = ROOT / "target" / "release" / "effect_converter"
OPTIMIZED_BIN_FALLBACK = ROOT / "crate" / "target" / "release" / "effect_converter"
EFF_BASE = REFS / "effect"

# Common local paths where Smash Ultimate effect archives are extracted.
EFFECT_SEARCH_PATHS = [
    Path("/home/leap/Workshop/Smash Mod Tools/ArcExplorer_linux_x64/export/effect"),
    Path.home() / "Workshop/Smash Mod Tools/ArcExplorer_linux_x64/export/effect",
    Path.home() / ".local/share/Trash/files/References/effect",
]


def log(msg: str) -> None:
    print(msg, flush=True)


def run(cmd: list[str], *, cwd: Path | None = None, env: dict | None = None) -> None:
    log(f"  $ {' '.join(cmd)}")
    subprocess.run(cmd, cwd=cwd, env=env, check=True)


def count_eff_files(path: Path) -> int:
    if not path.is_dir():
        return 0
    return sum(1 for _ in path.rglob("*.eff"))


def ensure_tmp() -> Path:
    TMP.mkdir(parents=True, exist_ok=True)
    return TMP


def work_dir(name: str) -> Path:
    path = ensure_tmp() / name
    path.mkdir(parents=True, exist_ok=True)
    return path


def ensure_published_crate_source() -> Path:
    marker = PUBLISHED_SRC / "Cargo.toml"
    if marker.is_file():
        return PUBLISHED_SRC

    log(f"Downloading effect_library {PUBLISHED_CRATE_VERSION} from crates.io...")
    REFS.mkdir(parents=True, exist_ok=True)
    crate_path = REFS / f"effect_library-{PUBLISHED_CRATE_VERSION}.crate"
    urllib.request.urlretrieve(PUBLISHED_CRATE_URL, crate_path)
    with tarfile.open(crate_path, "r:gz") as tar:
        tar.extractall(path=REFS)

    cargo_toml = PUBLISHED_SRC / "Cargo.toml"
    text = cargo_toml.read_text()
    if "[workspace]" not in text:
        cargo_toml.write_text(text.replace("\n[lib]", "\n[workspace]\n\n[lib]", 1))

    return PUBLISHED_SRC


def ensure_published_binary() -> Path:
    if PUBLISHED_BIN.is_file():
        return PUBLISHED_BIN

    log("Building published effect_library (crates.io) dumper...")
    src = ensure_published_crate_source()
    env = os.environ.copy()
    env["CARGO_TARGET_DIR"] = str(PUBLISHED_SRC / "target")
    run(
        ["cargo", "build", "--release", "--bin", "effect_dumper"],
        cwd=src,
        env=env,
    )
    if not PUBLISHED_BIN.is_file():
        raise RuntimeError(f"Published build did not produce {PUBLISHED_BIN}")
    return PUBLISHED_BIN


def ensure_csharp_repo() -> Path:
    if (CS_DIR / ".git").is_dir():
        return CS_DIR

    log(f"Cloning Joob's EffectLibrary fork ({CS_REPO})...")
    REFS.mkdir(parents=True, exist_ok=True)
    run(["git", "clone", "--depth", "1", CS_REPO, str(CS_DIR)])
    return CS_DIR


def ensure_csharp_binary() -> Path:
    for candidate in (CS_BIN, CS_BIN_FALLBACK):
        if candidate.is_file():
            return candidate

    if shutil.which("dotnet") is None:
        raise RuntimeError(
            "dotnet SDK not found. Install .NET 6+ to build EffectConverter:\n"
            "  https://dotnet.microsoft.com/download/dotnet/6.0"
        )

    log("Building C# EffectConverter (Joob fork)...")
    ensure_csharp_repo()
    run(
        ["dotnet", "publish", "-c", "Release", "-o", "bin/Release/publish"],
        cwd=CS_DIR / "EffectConverter",
    )
    for candidate in (CS_BIN, CS_BIN_FALLBACK):
        if candidate.is_file():
            return candidate
    raise RuntimeError(f"C# build did not produce {CS_BIN} or {CS_BIN_FALLBACK}")


def ensure_optimized_binary() -> Path:
    for candidate in (OPTIMIZED_BIN, OPTIMIZED_BIN_FALLBACK):
        if candidate.is_file():
            return candidate

    log("Building optimized effect_converter from current workspace...")
    env = os.environ.copy()
    env.setdefault("CARGO_TARGET_DIR", str(ROOT / "target"))
    run(
        ["cargo", "build", "--release", "--bin", "effect_converter"],
        cwd=ROOT / "crate",
        env=env,
    )
    for candidate in (OPTIMIZED_BIN, OPTIMIZED_BIN_FALLBACK):
        if candidate.is_file():
            return candidate
    raise RuntimeError(
        f"Optimized build did not produce {OPTIMIZED_BIN} or {OPTIMIZED_BIN_FALLBACK}"
    )


def resolve_effect_source() -> Path | None:
    if count_eff_files(EFF_BASE) > 0:
        return EFF_BASE.resolve()

    if EFF_BASE.is_symlink():
        target = EFF_BASE.resolve()
        if count_eff_files(target) > 0:
            return target

    for candidate in EFFECT_SEARCH_PATHS:
        if count_eff_files(candidate) > 0:
            return candidate.resolve()

    if os.environ.get("EFFECT_REFERENCE_PATH"):
        candidate = Path(os.environ["EFFECT_REFERENCE_PATH"]).expanduser()
        if count_eff_files(candidate) > 0:
            return candidate.resolve()

    return None


def ensure_effect_files() -> Path:
    existing = resolve_effect_source()
    if existing is None:
        raise RuntimeError(
            "No .eff files found for comparison.\n"
            "Provide game effect archives at one of:\n"
            f"  - {EFF_BASE}\n"
            f"  - $EFFECT_REFERENCE_PATH\n"
            "  - Smash Mod Tools ArcExplorer export (auto-detected if present)\n"
            "These files are not redistributable and cannot be downloaded automatically."
        )

    REFS.mkdir(parents=True, exist_ok=True)
    if EFF_BASE.exists() or EFF_BASE.is_symlink():
        if EFF_BASE.is_symlink() and EFF_BASE.resolve() == existing:
            return EFF_BASE
        if EFF_BASE.is_dir() and count_eff_files(EFF_BASE) > 0:
            return EFF_BASE
        if EFF_BASE.is_file():
            EFF_BASE.unlink()
        elif EFF_BASE.is_dir() and not any(EFF_BASE.iterdir()):
            EFF_BASE.rmdir()
        elif EFF_BASE.exists():
            raise RuntimeError(
                f"{EFF_BASE} exists but does not contain .eff files. "
                f"Remove it or symlink it to your effect export folder."
            )

    if existing != EFF_BASE.resolve():
        log(f"Linking {EFF_BASE} -> {existing}")
        EFF_BASE.symlink_to(existing, target_is_directory=True)

    count = count_eff_files(EFF_BASE)
    log(f"Using {count} .eff files from {EFF_BASE.resolve()}")
    return EFF_BASE


def ensure_compare_env(
    *,
    csharp: bool = False,
    published: bool = False,
    optimized: bool = True,
    effects: bool = True,
) -> dict[str, Path]:
    """Download/build everything needed for the requested comparison mode."""
    ensure_tmp()
    paths: dict[str, Path] = {}

    if effects:
        paths["effects"] = ensure_effect_files()
    if optimized:
        paths["optimized"] = ensure_optimized_binary()
    if published:
        paths["published"] = ensure_published_binary()
    if csharp:
        paths["csharp"] = ensure_csharp_binary()

    return paths


def main() -> int:
    import argparse

    parser = argparse.ArgumentParser(description="Prepare reference files for comparison scripts")
    parser.add_argument("--csharp", action="store_true", help="Also build C# EffectConverter")
    parser.add_argument("--published", action="store_true", help="Also build published Rust crate")
    parser.add_argument("--optimized", action="store_true", default=True, help="Build optimized Rust (default)")
    parser.add_argument("--all", action="store_true", help="Build everything")
    args = parser.parse_args()

    if args.all:
        args.csharp = True
        args.published = True
        args.optimized = True

    log("Setting up comparison environment...")
    paths = ensure_compare_env(
        csharp=args.csharp,
        published=args.published,
        optimized=args.optimized,
        effects=True,
    )
    log("\nReady:")
    for key, path in paths.items():
        log(f"  {key}: {path}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
