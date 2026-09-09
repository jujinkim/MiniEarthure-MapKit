#!/usr/bin/env python3
"""Audit the pure crate and public source tree without private game tooling."""
from pathlib import Path
import json
import subprocess
import tomllib
import re

root = Path(__file__).resolve().parents[1]
core = tomllib.loads((root / 'crates/mapkit-core/Cargo.toml').read_text())
allowed = {'serde', 'serde_json', 'sha2', 'schemars', 'libm'}
assert set(core['dependencies']) <= allowed, 'unexpected core dependency'
assert core['dependencies']['libm'] == '=0.2.16', 'portable math must stay exactly pinned'
for path in (root / 'crates/mapkit-core/src').rglob('*.rs'):
    source = path.read_text()
    for forbidden in ('std::fs', 'std::net', 'std::time', 'std::env', 'std::thread',
                      'std::arch', 'core::arch', 'SystemTime', 'godot::', 'reqwest', 'tokio::',
                      'HashMap', 'HashSet', 'rand::', 'getrandom', 'RandomState'):
        assert forbidden not in source, f'{path}: forbidden dependency {forbidden}'
    assert not re.search(r'\.\s*(sqrt|sin|cos|tan|atan2?|acos|asin|powf|exp|ln|log2|log10|round|floor|ceil|mul_add)\s*\(', source), f'{path}: use pinned portable math, not platform float intrinsics'
metadata = json.loads(subprocess.check_output(['cargo', 'metadata', '--locked', '--no-deps', '--format-version', '1', '--manifest-path', str(root / 'Cargo.toml')]))
for package in metadata['packages']:
    assert Path(package['manifest_path']).is_relative_to(root), 'workspace escapes public repository'
    for dependency in package['dependencies']:
        if dependency.get('path'):
            assert Path(dependency['path']).is_relative_to(root), 'private path dependency'
print('MapKit architecture: PASS')
