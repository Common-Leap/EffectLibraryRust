#!/usr/bin/env python3
"""Verify Rust folder rebuild matches C# EffectConverter byte-for-byte."""

from __future__ import annotations

import shutil
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from compare_setup import ensure_compare_env, work_dir  # noqa: E402


def roundtrip_one(eff_path: Path, rust: Path, csharp: Path, work: Path) -> dict:
    name = eff_path.stem
    cs_work = work / "csharp" / name
    cs_work.mkdir(parents=True, exist_ok=True)

    try:
        cs_dump = subprocess.run(
            [str(csharp), str(eff_path)],
            cwd=str(cs_work),
            capture_output=True,
            text=True,
            timeout=600,
        )
        if cs_dump.returncode != 0:
            return {
                "name": name,
                "status": "csharp_dump_fail",
                "err": (cs_dump.stderr or cs_dump.stdout)[-500:],
            }

        cs_folder = cs_work / name
        if not cs_folder.is_dir():
            return {"name": name, "status": "csharp_no_dump"}

        cs_build = subprocess.run(
            [str(csharp), str(cs_folder)],
            cwd=str(cs_work),
            capture_output=True,
            text=True,
            timeout=600,
        )
        if cs_build.returncode != 0:
            return {
                "name": name,
                "status": "csharp_build_fail",
                "err": (cs_build.stderr or cs_build.stdout)[-500:],
            }

        rust_build = subprocess.run(
            [str(rust), "build", str(cs_folder), str(cs_work / f"rust_{name}_NEW.eff")],
            capture_output=True,
            text=True,
            timeout=600,
        )
        if rust_build.returncode != 0:
            return {
                "name": name,
                "status": "rust_build_fail",
                "err": (rust_build.stderr or rust_build.stdout)[-500:],
            }

        cs_eff = cs_work / f"{name}_NEW.eff"
        rust_eff = cs_work / f"rust_{name}_NEW.eff"
        if not cs_eff.is_file():
            if rust_build.returncode == 0 and "Build skipped" in (rust_build.stdout or ""):
                return {"name": name, "status": "exact", "size": 0, "note": "header_only_skip"}
            return {"name": name, "status": "csharp_no_output"}
        if not rust_eff.is_file():
            return {"name": name, "status": "rust_no_output"}

        cs_data = cs_eff.read_bytes()
        rust_data = rust_eff.read_bytes()
        if cs_data == rust_data:
            return {"name": name, "status": "exact", "size": len(cs_data)}

        first_diff = next(
            (idx for idx, pair in enumerate(zip(cs_data, rust_data)) if pair[0] != pair[1]),
            min(len(cs_data), len(rust_data)),
        )
        return {
            "name": name,
            "status": "diff",
            "cs_size": len(cs_data),
            "rust_size": len(rust_data),
            "first_diff": first_diff,
        }
    finally:
        shutil.rmtree(cs_work, ignore_errors=True)


def main() -> int:
    print("Preparing round-trip test environment...")
    paths = ensure_compare_env(csharp=True, optimized=True)
    rust = paths["optimized"]
    csharp = paths["csharp"]
    effects = paths["effects"]

    work = work_dir("batch_roundtrip_work")
    if work.exists():
        shutil.rmtree(work)
    work.mkdir(parents=True)

    eff_files = sorted(effects.rglob("*.eff"))
    results = []
    total = len(eff_files)
    print(f"Round-tripping {total} .eff files (C# dump, Rust+C# rebuild)...")
    print(f"  C#:   {csharp}")
    print(f"  Rust: {rust}")
    print(f"  Root: {effects.resolve()}\n")

    for idx, eff in enumerate(eff_files, 1):
        try:
            res = roundtrip_one(eff, rust, csharp, work)
        except subprocess.TimeoutExpired:
            res = {"name": eff.stem, "status": "timeout"}
        except Exception as exc:
            res = {"name": eff.stem, "status": "error", "err": str(exc)}
        results.append(res)
        if idx % 25 == 0 or idx == total:
            exact = sum(1 for r in results if r["status"] == "exact")
            diff = sum(1 for r in results if r["status"] == "diff")
            fail = sum(1 for r in results if r["status"] not in ("exact", "diff"))
            print(f"[{idx}/{total}] exact={exact} diff={diff} fail={fail}", flush=True)

    exact = [r for r in results if r["status"] == "exact"]
    diffs = [r for r in results if r["status"] == "diff"]
    fails = [r for r in results if r["status"] not in ("exact", "diff")]

    print("\n=== FINAL ===")
    print(f"Total: {len(results)}")
    print(f"Exact: {len(exact)}")
    print(f"Diff:  {len(diffs)}")
    print(f"Fail:  {len(fails)}")

    if diffs:
        print("\nDiff effects:")
        for r in diffs[:20]:
            print(
                f"  {r['name']}: cs={r.get('cs_size')} rust={r.get('rust_size')} "
                f"first_diff={r.get('first_diff')}"
            )
        if len(diffs) > 20:
            print(f"  ... and {len(diffs) - 20} more")

    if fails:
        print("\nFailures:")
        for r in fails[:20]:
            detail = r.get("err", "")
            if detail:
                print(f"  {r['name']}: {r['status']} ({detail[:120]})")
            else:
                print(f"  {r['name']}: {r['status']}")
        if len(fails) > 20:
            print(f"  ... and {len(fails) - 20} more")

    shutil.rmtree(work, ignore_errors=True)
    return 1 if diffs or fails else 0


if __name__ == "__main__":
    sys.exit(main())
