#!/usr/bin/env python3
"""Compare crates.io effect_library 1.0.0 dump output vs local optimized build."""

import filecmp
import os
import shutil
import subprocess
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from compare_setup import ensure_compare_env, work_dir  # noqa: E402


def compare_dirs(a: Path, b: Path) -> dict:
    a_files, b_files = {}, {}
    for dp, _, fs in os.walk(a):
        for f in fs:
            p = Path(dp) / f
            a_files[str(p.relative_to(a))] = p
    for dp, _, fs in os.walk(b):
        for f in fs:
            p = Path(dp) / f
            b_files[str(p.relative_to(b))] = p

    all_rels = sorted(set(a_files) | set(b_files))
    only_a = [r for r in all_rels if r not in b_files]
    only_b = [r for r in all_rels if r not in a_files]
    mismatches = []
    matches = 0
    for rel in all_rels:
        if rel in a_files and rel in b_files:
            if filecmp.cmp(a_files[rel], b_files[rel], shallow=False):
                matches += 1
            else:
                mismatches.append(rel)
    return {
        "total": len(all_rels),
        "matches": matches,
        "only_published": only_a,
        "only_optimized": only_b,
        "mismatches": mismatches,
        "exact": not only_a and not only_b and not mismatches,
    }


def run_one(eff_path: Path, published: Path, optimized: Path, work: Path) -> dict:
    name = eff_path.stem
    pub_out = work / "published" / name
    opt_out = work / "optimized" / name
    pub_out.mkdir(parents=True, exist_ok=True)
    opt_out.mkdir(parents=True, exist_ok=True)

    try:
        t0 = time.perf_counter()
        pub = subprocess.run(
            [str(published), "dump", str(eff_path), str(pub_out)],
            capture_output=True,
            text=True,
            timeout=600,
        )
        pub_secs = time.perf_counter() - t0
        if pub.returncode != 0:
            return {
                "name": name,
                "status": "published_fail",
                "err": (pub.stderr or pub.stdout)[-500:],
            }

        t0 = time.perf_counter()
        opt = subprocess.run(
            [str(optimized), "dump", str(eff_path), str(opt_out)],
            capture_output=True,
            text=True,
            timeout=600,
        )
        opt_secs = time.perf_counter() - t0
        if opt.returncode != 0:
            return {
                "name": name,
                "status": "optimized_fail",
                "err": (opt.stderr or opt.stdout)[-500:],
            }

        cmp = compare_dirs(pub_out, opt_out)
        speedup_pct = ((pub_secs - opt_secs) / pub_secs * 100.0) if pub_secs > 0 else 0.0
        return {
            "name": name,
            "status": "exact" if cmp["exact"] else "diff",
            "published_secs": pub_secs,
            "optimized_secs": opt_secs,
            "speedup_pct": speedup_pct,
            **cmp,
        }
    finally:
        shutil.rmtree(pub_out, ignore_errors=True)
        shutil.rmtree(opt_out, ignore_errors=True)


def main() -> int:
    print("Preparing comparison environment...")
    paths = ensure_compare_env(published=True, optimized=True)
    published = paths["published"]
    optimized = paths["optimized"]
    effects = paths["effects"]

    work = work_dir("compare_pub_opt_work")
    if work.exists():
        shutil.rmtree(work)
    work.mkdir(parents=True)

    eff_files = sorted(effects.rglob("*.eff"))
    print(f"Published: {published}")
    print(f"Optimized: {optimized}")
    print(f"Comparing {len(eff_files)} .eff files...\n")

    results = []
    total = len(eff_files)
    for idx, eff in enumerate(eff_files, 1):
        try:
            res = run_one(eff, published, optimized, work)
        except subprocess.TimeoutExpired:
            res = {"name": eff.stem, "status": "timeout"}
        except Exception as e:
            res = {"name": eff.stem, "status": "error", "err": str(e)}
        results.append(res)
        if idx % 25 == 0 or idx == total:
            exact = sum(1 for r in results if r.get("status") == "exact")
            diff = sum(1 for r in results if r.get("status") == "diff")
            fail = sum(1 for r in results if r.get("status") not in ("exact", "diff"))
            print(f"[{idx}/{total}] exact={exact} diff={diff} fail={fail}", flush=True)

    exact = [r for r in results if r.get("status") == "exact"]
    diffs = [r for r in results if r.get("status") == "diff"]
    fails = [r for r in results if r.get("status") not in ("exact", "diff")]

    pub_total = sum(r["published_secs"] for r in exact)
    opt_total = sum(r["optimized_secs"] for r in exact)
    overall_speedup = ((pub_total - opt_total) / pub_total * 100.0) if pub_total > 0 else 0.0

    print(f"\n=== ACCURACY ===")
    print(f"Total effects: {len(results)}")
    print(f"Exact match:   {len(exact)}")
    print(f"Diff:          {len(diffs)}")
    print(f"Fail:          {len(fails)}")

    if diffs:
        print("\nDiff effects:")
        for r in diffs[:20]:
            print(
                f"  {r['name']}: {len(r.get('mismatches', []))} mismatches, "
                f"pub-only={len(r.get('only_published', []))}, opt-only={len(r.get('only_optimized', []))}"
            )

    if fails:
        print("\nFailures:")
        for r in fails[:20]:
            print(f"  {r['name']}: {r['status']}")

    print(f"\n=== SPEED (exact matches only) ===")
    print(f"Published total: {pub_total:.2f}s")
    print(f"Optimized total: {opt_total:.2f}s")
    print(f"Overall faster:  {overall_speedup:.1f}%")

    if exact:
        per_effect = sorted(exact, key=lambda r: r["speedup_pct"], reverse=True)
        avg_speedup = sum(r["speedup_pct"] for r in exact) / len(exact)
        print(f"Mean per-effect speedup: {avg_speedup:.1f}%")
        print("\nTop 5 speedups:")
        for r in per_effect[:5]:
            print(
                f"  {r['name']}: {r['published_secs']:.3f}s -> {r['optimized_secs']:.3f}s "
                f"({r['speedup_pct']:.1f}% faster, {r['total']} files)"
            )

    shutil.rmtree(work, ignore_errors=True)
    print("\nTemp outputs deleted.")
    return 1 if diffs or fails else 0


if __name__ == "__main__":
    sys.exit(main())
