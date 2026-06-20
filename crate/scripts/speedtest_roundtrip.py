#!/usr/bin/env python3
"""Speedtest decompile (dump) and recompile (build) for selected .eff files."""

from __future__ import annotations

import argparse
import shutil
import statistics
import subprocess
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from compare_setup import ensure_compare_env, work_dir  # noqa: E402

DEFAULT_EFFECTS = [
    "ef_matchup",
    "ef_sable",
    "ef_mario",
    "ef_fox",
    "ef_koopa",
    "ef_brave",
    "ef_common",
    "ef_item",
    "ef_dracula",
    "ef_marx",
]


def find_eff(effects_root: Path, name: str) -> Path | None:
    matches = sorted(effects_root.rglob(f"{name}.eff"))
    return matches[0] if matches else None


def median_secs(fn, iterations: int) -> float:
    samples = [fn() for _ in range(iterations)]
    return statistics.median(samples)


def bench_rust(
    converter: Path,
    eff: Path,
    work: Path,
    iterations: int,
) -> tuple[float, float, int]:
    name = eff.stem
    dump_dir = work / "rust" / name
    out_eff = work / "rust" / f"{name}_NEW.eff"
    dump_dir.parent.mkdir(parents=True, exist_ok=True)

    def dump_once() -> float:
        if dump_dir.exists():
            shutil.rmtree(dump_dir)
        t0 = time.perf_counter()
        proc = subprocess.run(
            [str(converter), "dump", str(eff), str(dump_dir)],
            capture_output=True,
            text=True,
            timeout=600,
        )
        if proc.returncode != 0:
            raise RuntimeError((proc.stderr or proc.stdout)[-500:])
        return time.perf_counter() - t0

    def build_once() -> float:
        if out_eff.exists():
            out_eff.unlink()
        t0 = time.perf_counter()
        proc = subprocess.run(
            [str(converter), "build", str(dump_dir), str(out_eff)],
            capture_output=True,
            text=True,
            timeout=600,
        )
        if proc.returncode != 0:
            raise RuntimeError((proc.stderr or proc.stdout)[-500:])
        return time.perf_counter() - t0

    dump_secs = median_secs(dump_once, iterations)
    build_secs = median_secs(build_once, iterations)
    size = out_eff.stat().st_size if out_eff.is_file() else eff.stat().st_size
    return dump_secs, build_secs, size


def bench_csharp(
    converter: Path,
    eff: Path,
    work: Path,
    iterations: int,
) -> tuple[float, float, int]:
    name = eff.stem
    cs_work = work / "csharp" / name
    cs_work.mkdir(parents=True, exist_ok=True)
    out_eff = cs_work / f"{name}_NEW.eff"

    def roundtrip_once() -> tuple[float, float]:
        if (cs_work / name).exists():
            shutil.rmtree(cs_work / name)
        if out_eff.exists():
            out_eff.unlink()

        t0 = time.perf_counter()
        proc = subprocess.run(
            [str(converter), str(eff)],
            cwd=str(cs_work),
            capture_output=True,
            text=True,
            timeout=600,
        )
        if proc.returncode != 0:
            raise RuntimeError((proc.stderr or proc.stdout)[-500:])
        dump_secs = time.perf_counter() - t0

        folder = cs_work / name
        if not folder.is_dir():
            raise RuntimeError("C# dump produced no folder")

        t0 = time.perf_counter()
        proc = subprocess.run(
            [str(converter), str(folder)],
            cwd=str(cs_work),
            capture_output=True,
            text=True,
            timeout=600,
        )
        if proc.returncode != 0:
            raise RuntimeError((proc.stderr or proc.stdout)[-500:])
        build_secs = time.perf_counter() - t0
        return dump_secs, build_secs

    dump_samples: list[float] = []
    build_samples: list[float] = []
    for _ in range(iterations):
        dump_secs, build_secs = roundtrip_once()
        dump_samples.append(dump_secs)
        build_samples.append(build_secs)

    size = out_eff.stat().st_size if out_eff.is_file() else eff.stat().st_size
    return statistics.median(dump_samples), statistics.median(build_samples), size


def fmt_mb(num_bytes: int) -> str:
    return f"{num_bytes / (1024 * 1024):.2f}"


def pct_faster(old: float, new: float) -> str:
    if old <= 0:
        return "—"
    return f"{(old - new) / old * 100:+.1f}%"


def main() -> int:
    parser = argparse.ArgumentParser(description="Speedtest dump/build roundtrip on .eff files")
    parser.add_argument(
        "--effects",
        nargs="*",
        default=DEFAULT_EFFECTS,
        help="Effect base names (default: representative sample)",
    )
    parser.add_argument(
        "--iterations",
        type=int,
        default=3,
        help="Median timing iterations per effect (default: 3)",
    )
    parser.add_argument(
        "--csharp",
        action="store_true",
        help="Also benchmark C# EffectConverter",
    )
    parser.add_argument(
        "--all-found",
        action="store_true",
        help="Use all named effects that exist under the effect root",
    )
    args = parser.parse_args()

    print("Preparing speedtest environment...")
    paths = ensure_compare_env(csharp=args.csharp, optimized=True)
    rust = paths["optimized"]
    csharp = paths.get("csharp")
    effects_root = paths["effects"]
    work = work_dir("speedtest_roundtrip_work")
    if work.exists():
        shutil.rmtree(work)
    work.mkdir(parents=True)

    names = args.effects
    if args.all_found:
        names = sorted({p.stem for p in effects_root.rglob("*.eff")})

    print(f"Rust:   {rust}")
    if args.csharp and csharp:
        print(f"C#:     {csharp}")
    print(f"Root:   {effects_root.resolve()}")
    print(f"Runs:   {args.iterations} median iterations per stage\n")

    rows: list[dict] = []
    missing: list[str] = []

    for name in names:
        eff = find_eff(effects_root, name)
        if eff is None:
            missing.append(name)
            continue
        row: dict = {"name": name, "input_mb": fmt_mb(eff.stat().st_size)}
        try:
            dump, build, out_size = bench_rust(rust, eff, work, args.iterations)
            row.update(
                {
                    "rust_dump": dump,
                    "rust_build": build,
                    "rust_total": dump + build,
                    "out_mb": fmt_mb(out_size),
                    "status": "ok",
                }
            )
            if args.csharp and csharp:
                cs_dump, cs_build, _ = bench_csharp(csharp, eff, work, args.iterations)
                row["cs_dump"] = cs_dump
                row["cs_build"] = cs_build
                row["cs_total"] = cs_dump + cs_build
        except subprocess.TimeoutExpired:
            row["status"] = "timeout"
        except Exception as exc:
            row["status"] = "error"
            row["err"] = str(exc)
        rows.append(row)
        if row.get("status") == "ok":
            print(
                f"{name:<16} dump={row['rust_dump']:.3f}s build={row['rust_build']:.3f}s "
                f"total={row['rust_total']:.3f}s ({row['input_mb']} MiB in)",
                flush=True,
            )
        else:
            print(f"{name:<16} {row['status']}: {row.get('err', '')[:80]}", flush=True)

    ok = [r for r in rows if r.get("status") == "ok"]
    if not ok:
        print("\nNo successful timings.")
        shutil.rmtree(work, ignore_errors=True)
        return 1

    rust_dump_total = sum(r["rust_dump"] for r in ok)
    rust_build_total = sum(r["rust_build"] for r in ok)
    rust_total = sum(r["rust_total"] for r in ok)

    print("\n=== RUST SUMMARY ===")
    print(f"Effects timed: {len(ok)}")
    print(f"Dump total:    {rust_dump_total:.2f}s")
    print(f"Build total:   {rust_build_total:.2f}s")
    print(f"Roundtrip:     {rust_total:.2f}s")
    print(f"Build share:   {rust_build_total / rust_total * 100:.1f}% of roundtrip")

    print("\n=== RUST DETAIL ===")
    header = f"{'Effect':<16} {'In MiB':>7} {'Dump':>8} {'Build':>8} {'Total':>8} {'Build%':>7}"
    print(header)
    print("-" * len(header))
    for r in sorted(ok, key=lambda item: item["rust_total"], reverse=True):
        build_pct = r["rust_build"] / r["rust_total"] * 100
        print(
            f"{r['name']:<16} {r['input_mb']:>7} "
            f"{r['rust_dump']:8.3f} {r['rust_build']:8.3f} {r['rust_total']:8.3f} {build_pct:6.1f}%"
        )

    if args.csharp and ok and ok[0].get("cs_total") is not None:
        cs_dump_total = sum(r["cs_dump"] for r in ok)
        cs_build_total = sum(r["cs_build"] for r in ok)
        cs_total = sum(r["cs_total"] for r in ok)
        print("\n=== C# vs RUST (median per effect) ===")
        print(f"{'Effect':<16} {'C# tot':>8} {'Rust tot':>9} {'Δ total':>9} {'C# dump':>8} {'Rust dump':>10} {'C# build':>9} {'Rust build':>10}")
        print("-" * 92)
        for r in sorted(ok, key=lambda item: item["rust_total"], reverse=True):
            print(
                f"{r['name']:<16} {r['cs_total']:8.3f} {r['rust_total']:9.3f} "
                f"{pct_faster(r['cs_total'], r['rust_total']):>9} "
                f"{r['cs_dump']:8.3f} {r['rust_dump']:10.3f} "
                f"{r['cs_build']:9.3f} {r['rust_build']:10.3f}"
            )
        print("\n=== AGGREGATE C# vs RUST ===")
        print(f"C# roundtrip:   {cs_total:.2f}s")
        print(f"Rust roundtrip: {rust_total:.2f}s ({pct_faster(cs_total, rust_total)} vs C#)")
        print(f"  dump:  C# {cs_dump_total:.2f}s  Rust {rust_dump_total:.2f}s ({pct_faster(cs_dump_total, rust_dump_total)})")
        print(f"  build: C# {cs_build_total:.2f}s  Rust {rust_build_total:.2f}s ({pct_faster(cs_build_total, rust_build_total)})")

    if missing:
        print(f"\nMissing ({len(missing)}): {', '.join(missing[:10])}" + (" ..." if len(missing) > 10 else ""))

    shutil.rmtree(work, ignore_errors=True)
    return 0 if len(ok) == len(names) or not args.all_found else 1


if __name__ == "__main__":
    sys.exit(main())
