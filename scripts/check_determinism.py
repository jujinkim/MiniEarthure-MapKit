#!/usr/bin/env python3
"""Compare native core vectors across fresh processes/build profiles/OS exports."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--release', action='store_true')
    parser.add_argument('--output', type=Path, help='Write comparable vectors to a new file')
    args = parser.parse_args()
    expected = json.loads((ROOT / 'spec/determinism-vectors.json').read_text(encoding='utf-8'))
    command = ['cargo', 'run', '--quiet', '--locked', '--manifest-path', str(ROOT / 'Cargo.toml'),
               '-p', 'mapkit-core', '--example', 'determinism']
    if args.release:
        command.append('--release')
    previous = None
    for zone in ('UTC', 'Pacific/Honolulu'):
        result = subprocess.run(command, env=dict(os.environ, TZ=zone, LANG='C', LC_ALL='C'),
                                capture_output=True, timeout=300)
        if result.returncode:
            raise SystemExit(result.stderr.decode(errors='replace'))
        actual = json.loads(result.stdout)
        if actual != expected:
            for a, e in zip(actual.get('fixtures', []), expected['fixtures']):
                if a != e:
                    raise SystemExit(f'Determinism mismatch: {e["name"]}; compare cell/component vectors. '
                                     'A changed golden requires an explicit version decision.')
            raise SystemExit('Determinism vector schema/count mismatch')
        assert previous is None or previous == result.stdout, 'fresh process/timezone byte mismatch'
        previous = result.stdout
    if args.output:
        with args.output.open('xb') as output:
            output.write(previous)
    print(f'Determinism: PASS ({len(expected["fixtures"])} fixtures, '
          f'{sum(len(f["cells"]) for f in expected["fixtures"])} cells, two fresh processes; '
          f'vectors SHA256 {hashlib.sha256(previous).hexdigest()})')


if __name__ == '__main__':
    main()
