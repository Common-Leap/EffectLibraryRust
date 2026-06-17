#!/usr/bin/env python3
import os, subprocess, shutil, filecmp, json, sys
from pathlib import Path

BASE = Path(__file__).resolve().parents[2] / 'References' / 'effect'
RUST = Path(__file__).resolve().parents[1] / 'target' / 'release' / 'effect_dumper'
CS = Path(__file__).resolve().parents[2] / 'References' / 'EffectLibrary' / 'EffectConverter' / 'bin' / 'Release' / 'EffectConverter'
WORK = Path(__file__).resolve().parents[1] / '.batch_eff_work'


def compare_one(eff_path: Path):
    name = eff_path.stem
    rust_out = WORK / 'rust' / name
    cs_work = WORK / 'csharp' / name
    rust_out.mkdir(parents=True, exist_ok=True)
    cs_work.mkdir(parents=True, exist_ok=True)
    try:
        r = subprocess.run([str(RUST), 'dump', str(eff_path), str(rust_out)], capture_output=True, text=True, timeout=180)
        if r.returncode != 0:
            return {'name': name, 'status': 'rust_fail', 'err': (r.stderr or r.stdout)[-500:]}
        c = subprocess.run([str(CS), str(eff_path)], cwd=str(cs_work), capture_output=True, text=True, timeout=180)
        if c.returncode != 0:
            return {'name': name, 'status': 'csharp_fail', 'err': (c.stderr or c.stdout)[-500:]}
        cs_out = cs_work / name
        if not cs_out.is_dir():
            return {'name': name, 'status': 'csharp_no_output'}
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
                    mismatches.append({'file': rel, 'rust_size': rust_files[rel].stat().st_size, 'cs_size': cs_files[rel].stat().st_size})
        if not only_cs and not only_rust and not mismatches:
            return {'name': name, 'status': 'exact', 'files': matches}
        return {'name': name, 'status': 'diff', 'files_total': len(all_rels), 'matches': matches,
                'only_csharp_n': len(only_cs), 'only_rust_n': len(only_rust), 'only_csharp': only_cs[:10],
                'only_rust': only_rust[:10], 'mismatches': mismatches[:10], 'mismatch_n': len(mismatches)}
    finally:
        shutil.rmtree(rust_out, ignore_errors=True)
        shutil.rmtree(cs_work, ignore_errors=True)


def main():
    eff_files = sorted(BASE.rglob('*.eff'))
    if WORK.exists():
        shutil.rmtree(WORK)
    WORK.mkdir(parents=True)
    results = []
    total = len(eff_files)
    print(f'Comparing {total} .eff files...')
    for idx, eff in enumerate(eff_files, 1):
        try:
            res = compare_one(eff)
        except subprocess.TimeoutExpired:
            res = {'name': eff.stem, 'status': 'timeout'}
        except Exception as e:
            res = {'name': eff.stem, 'status': 'error', 'err': str(e)}
        results.append(res)
        if idx % 25 == 0 or idx == total:
            e = sum(1 for r in results if r['status'] == 'exact')
            d = sum(1 for r in results if r['status'] == 'diff')
            f = sum(1 for r in results if r['status'] not in ('exact', 'diff'))
            print(f'[{idx}/{total}] exact={e} diff={d} fail={f}', flush=True)
    exact = [r for r in results if r['status'] == 'exact']
    diffs = [r for r in results if r['status'] == 'diff']
    fails = [r for r in results if r['status'] not in ('exact', 'diff')]
    print(f'\n=== FINAL ===\nTotal: {len(results)}\nExact: {len(exact)}\nDiff: {len(diffs)}\nFail: {len(fails)}')
    if diffs:
        print('\nDiff effects:')
        for r in diffs:
            print(f"  {r['name']}: {r.get('mismatch_n',0)} mismatch, cs-only={r.get('only_csharp_n',0)}, rust-only={r.get('only_rust_n',0)}")
    if fails:
        print('\nFailures:')
        for r in fails:
            print(f"  {r['name']}: {r['status']}")
    shutil.rmtree(WORK, ignore_errors=True)
    print('\nOutputs deleted.')
    return 1 if diffs or fails else 0


if __name__ == '__main__':
    sys.exit(main())
