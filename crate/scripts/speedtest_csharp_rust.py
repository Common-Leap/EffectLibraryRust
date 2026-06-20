#!/usr/bin/env python3
"""Speedtest: Joob's C# EffectConverter vs published Rust vs optimized Rust."""

import shutil
import subprocess
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from compare_setup import ensure_compare_env, work_dir  # noqa: E402


def run_csharp(eff_path: Path, csharp: Path, out_root: Path) -> float:
    name = eff_path.stem
    work = out_root / "csharp" / name
    work.mkdir(parents=True, exist_ok=True)
    t0 = time.perf_counter()
    proc = subprocess.run(
        [str(csharp), str(eff_path)],
        cwd=str(work),
        capture_output=True,
        text=True,
        timeout=600,
    )
    secs = time.perf_counter() - t0
    if proc.returncode != 0:
        raise RuntimeError((proc.stderr or proc.stdout)[-500:])
    dumped = work / name
    if not dumped.is_dir():
        raise RuntimeError("csharp produced no output directory")
    final = out_root / "csharp_out" / name
    if final.exists():
        shutil.rmtree(final)
    shutil.move(str(dumped), str(final))
    return secs


def run_rust(converter: Path, eff_path: Path, out_root: Path, label: str) -> float:
    name = eff_path.stem
    out = out_root / f"{label}_out" / name
    out.mkdir(parents=True, exist_ok=True)
    t0 = time.perf_counter()
    proc = subprocess.run(
        [str(converter), "dump", str(eff_path), str(out)],
        capture_output=True,
        text=True,
        timeout=600,
    )
    secs = time.perf_counter() - t0
    if proc.returncode != 0:
        raise RuntimeError((proc.stderr or proc.stdout)[-500:])
    return secs


def pct_faster(old_secs: float, new_secs: float) -> float:
    if old_secs <= 0:
        return 0.0
    return (old_secs - new_secs) / old_secs * 100.0


def main() -> int:
    print("Preparing comparison environment...")
    paths = ensure_compare_env(csharp=True, published=True, optimized=True)
    csharp = paths["csharp"]
    published = paths["published"]
    optimized = paths["optimized"]
    effects = paths["effects"]

    work = work_dir("speedtest_cs_rust_work")
    if work.exists():
        shutil.rmtree(work)
    work.mkdir(parents=True)

    eff_files = sorted(effects.rglob("*.eff"))
    print(f"C#:              {csharp}")
    print(f"Published Rust:  {published}")
    print(f"Optimized Rust:  {optimized}")
    print(f"Timing {len(eff_files)} .eff files...\n")

    results = []
    total = len(eff_files)
    for idx, eff in enumerate(eff_files, 1):
        name = eff.stem
        row = {"name": name}
        try:
            row["csharp_secs"] = run_csharp(eff, csharp, work)
            row["published_secs"] = run_rust(published, eff, work, "published")
            row["optimized_secs"] = run_rust(optimized, eff, work, "optimized")
            row["status"] = "ok"
        except subprocess.TimeoutExpired:
            row["status"] = "timeout"
        except Exception as e:
            row["status"] = "error"
            row["err"] = str(e)
        results.append(row)
        if idx % 25 == 0 or idx == total:
            ok = sum(1 for r in results if r.get("status") == "ok")
            print(f"[{idx}/{total}] ok={ok} fail={idx - ok}", flush=True)

    ok_rows = [r for r in results if r.get("status") == "ok"]
    fails = [r for r in results if r.get("status") != "ok"]

    cs_total = sum(r["csharp_secs"] for r in ok_rows)
    pub_total = sum(r["published_secs"] for r in ok_rows)
    opt_total = sum(r["optimized_secs"] for r in ok_rows)

    print("\n=== TOTAL WALL TIME (all effects) ===")
    print(f"C# (Joob fork):     {cs_total:8.2f}s")
    print(f"Published Rust:     {pub_total:8.2f}s  ({pct_faster(cs_total, pub_total):+.1f}% vs C#)")
    print(f"Optimized Rust:     {opt_total:8.2f}s  ({pct_faster(cs_total, opt_total):+.1f}% vs C#)")
    print(f"Optimized vs Pub:   {pct_faster(pub_total, opt_total):+.1f}% faster")

    showcase = ["ef_mario", "ef_common", "ef_item", "ef_fox", "ef_kirby", "ef_trail", "ef_standard"]
    by_name = {r["name"]: r for r in ok_rows}

    print("\n=== DETAILED TABLE (selected effects, seconds) ===")
    print(f"{'Effect':<18} {'C#':>8} {'Rust 1.0':>9} {'Optimized':>10} {'C#→Rust':>8} {'Rust→Opt':>9} {'C#→Opt':>8}")
    print("-" * 78)
    for name in showcase:
        if name not in by_name:
            continue
        r = by_name[name]
        print(
            f"{name:<18} "
            f"{r['csharp_secs']:8.3f} "
            f"{r['published_secs']:9.3f} "
            f"{r['optimized_secs']:10.3f} "
            f"{pct_faster(r['csharp_secs'], r['published_secs']):7.1f}% "
            f"{pct_faster(r['published_secs'], r['optimized_secs']):8.1f}% "
            f"{pct_faster(r['csharp_secs'], r['optimized_secs']):7.1f}%"
        )

    print("\n=== AGGREGATE SUMMARY TABLE ===")
    print("| Stage | Total time | vs C# | vs previous stage |")
    print("|-------|------------|-------|-------------------|")
    print(f"| C# (Joob fork) | {cs_total:.2f}s | — | — |")
    print(
        f"| Published Rust (crates.io 1.0.0) | {pub_total:.2f}s | "
        f"{pct_faster(cs_total, pub_total):.1f}% faster | "
        f"{pct_faster(cs_total, pub_total):.1f}% faster than C# |"
    )
    print(
        f"| Optimized Rust (current) | {opt_total:.2f}s | "
        f"{pct_faster(cs_total, opt_total):.1f}% faster | "
        f"{pct_faster(pub_total, opt_total):.1f}% faster than published |"
    )

    if fails:
        print(f"\nFailures ({len(fails)}):")
        for r in fails[:10]:
            print(f"  {r['name']}: {r.get('status')} {r.get('err', '')[:80]}")

    shutil.rmtree(work, ignore_errors=True)
    print("\nTemp outputs deleted.")
    return 1 if fails else 0


if __name__ == "__main__":
    sys.exit(main())
