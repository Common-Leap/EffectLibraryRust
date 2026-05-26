#!/usr/bin/env python3
import sys
sys.path.insert(0, '/home/leap/Workshop/EffectLibraryRust')

# Read the EFF file and dump a test
import subprocess
result = subprocess.run([
    '/home/leap/Workshop/EffectLibraryRust/crate/target/debug/effect_dumper',
    'dump',
    '/home/leap/Workshop/EffectLibraryRust/EFF and baseline/ef_samus.eff',
    '/tmp/test_dump_debug'
], capture_output=True, text=True)

print("STDOUT:")
print(result.stdout)
print("\nSTDERR:")
print(result.stderr)

# List what was created
import os
print("\nCreated files:")
for root, dirs, files in os.walk('/tmp/test_dump_debug'):
    level = root.replace('/tmp/test_dump_debug', '').count(os.sep)
    indent = ' ' * 2 * level
    print(f'{indent}{os.path.basename(root)}/')
    subindent = ' ' * 2 * (level + 1)
    for file in files[:10]:  # Limit to first 10 files per directory
        print(f'{subindent}{file}')
    if len(files) > 10:
        print(f'{subindent}... and {len(files) - 10} more files')
    if level > 3:  # Limit recursion
        break
