#!/usr/bin/env python3
import filecmp
import os
import shutil
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from compare_setup import ensure_compare_env, work_dir  # noqa: E402


def compare_one(eff_path: Path, rust: Path, csharp: Path, work: Path):
    name = eff_path.stem
    rust_out = work / "rust" / name
    cs_work = work / "csharp" / name
    rust_out.mkdir(parents=True, exist_ok=True)
    cs_work.mkdir(parents=True, exist_ok=True)
    try:
        r = subprocess.run(
            [str(rust), "dump", str(eff_path), str(rust_out)],
            capture_output=True,
            text=True,
            timeout=600,
        )
        if r.returncode != 0:
            return {"name": name, "status": "rust_fail", "err": (r.stderr or r.stdout)[-500:]}
        c = subprocess.run(
            [str(csharp), str(eff_path)],
            cwd=str(cs_work),
            capture_output=True,
            text=True,
            timeout=600,
        )
        if c.returncode != 0:
            return {"name": name, "status": "csharp_fail", "err": (c.stderr or c.stdout)[-500:]}
        cs_out = cs_work / name
        if not cs_out.is_dir():
            return {"name": name, "status": "csharp_no_output"}
        rust_files, cs_files = {}, {}
        for dp, _, fs in os.walk(rust_out):
            for f in fs:
                p = Path(dp) / f
                rust_files[str(p.relative_to(rust_out))] = p
        for dp, _, fs in os.walk(cs_out):
            for f in fs:
                p = Path(dp) / f
                cs_files[str(p.relative_to(cs_out))] = p
        all_rels = sorted(set(rust_files) | set(cs_files))
        only_cs = [rel for rel in all_rels if rel not in rust_files]
        only_rust = [rel for rel in all_rels if rel not in cs_files]
        mismatches, matches = [], 0
        for rel in all_rels:
            if rel in rust_files and rel in cs_files:
                if filecmp.cmp(rust_files[rel], cs_files[rel], shallow=False):
                    matches += 1
                else:
                    mismatches.append(
                        {
                            "file": rel,
                            "rust_size": rust_files[rel].stat().st_size,
                            "cs_size": cs_files[rel].stat().st_size,
                        }
                    )
        if not only_cs and not only_rust and not mismatches:
            return {"name": name, "status": "exact", "files": matches}
        return {
            "name": name,
            "status": "diff",
            "files_total": len(all_rels),
            "matches": matches,
            "only_csharp_n": len(only_cs),
            "only_rust_n": len(only_rust),
            "only_csharp": only_cs[:10],
            "only_rust": only_rust[:10],
            "mismatches": mismatches[:10],
            "mismatch_n": len(mismatches),
        }
    finally:
        shutil.rmtree(rust_out, ignore_errors=True)
        shutil.rmtree(cs_work, ignore_errors=True)


def main():
    print("Preparing comparison environment...")
    paths = ensure_compare_env(csharp=True, optimized=True)
    rust = paths["optimized"]
    csharp = paths["csharp"]
    effects = paths["effects"]

    work = work_dir("batch_eff_work")
    if work.exists():
        shutil.rmtree(work)
    work.mkdir(parents=True)

    eff_files = sorted(effects.rglob("*.eff"))
    results = []
    total = len(eff_files)
    print(f"Comparing {total} .eff files...")
    print(f"  C#:      {csharp}")
    print(f"  Rust:    {rust}")
    print(f"  Effects: {effects.resolve()}\n")

    for idx, eff in enumerate(eff_files, 1):
        try:
            res = compare_one(eff, rust, csharp, work)
        except subprocess.TimeoutExpired:
            res = {"name": eff.stem, "status": "timeout"}
        except Exception as e:
            res = {"name": eff.stem, "status": "error", "err": str(e)}
        results.append(res)
        if idx % 25 == 0 or idx == total:
            e = sum(1 for r in results if r["status"] == "exact")
            d = sum(1 for r in results if r["status"] == "diff")
            f = sum(1 for r in results if r["status"] not in ("exact", "diff"))
            print(f"[{idx}/{total}] exact={e} diff={d} fail={f}", flush=True)

    exact = [r for r in results if r["status"] == "exact"]
    diffs = [r for r in results if r["status"] == "diff"]
    fails = [r for r in results if r["status"] not in ("exact", "diff")]
    print(f"\n=== FINAL ===\nTotal: {len(results)}\nExact: {len(exact)}\nDiff: {len(diffs)}\nFail: {len(fails)}")
    if diffs:
        print("\nDiff effects:")
        for r in diffs:
            print(
                f"  {r['name']}: {r.get('mismatch_n', 0)} mismatch, "
                f"cs-only={r.get('only_csharp_n', 0)}, rust-only={r.get('only_rust_n', 0)}"
            )
    if fails:
        print("\nFailures:")
        for r in fails:
            print(f"  {r['name']}: {r['status']}")
    shutil.rmtree(work, ignore_errors=True)
    print("\nOutputs deleted.")
    return 1 if diffs or fails else 0


if __name__ == "__main__":
    sys.exit(main())
