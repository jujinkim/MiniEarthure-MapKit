#!/usr/bin/env python3
"""Audit the pure crate and public source tree without private game tooling."""
from pathlib import Path
import json
import subprocess
import tomllib

root = Path(__file__).resolve().parents[1]
core = tomllib.loads((root / 'crates/mapkit-core/Cargo.toml').read_text())
allowed = {'serde', 'serde_json', 'sha2', 'schemars', 'libm'}
assert set(core['dependencies']) <= allowed, 'unexpected core dependency'
for path in (root / 'crates/mapkit-core/src').rglob('*.rs'):
    source = path.read_text()
    for forbidden in ('std::fs', 'std::net', 'std::time', 'SystemTime', 'godot::', 'reqwest', 'tokio::'):
        assert forbidden not in source, f'{path}: forbidden dependency {forbidden}'
metadata = json.loads(subprocess.check_output(['cargo', 'metadata', '--locked', '--no-deps', '--format-version', '1', '--manifest-path', str(root / 'Cargo.toml')]))
for package in metadata['packages']:
    assert Path(package['manifest_path']).is_relative_to(root), 'workspace escapes public repository'
    for dependency in package['dependencies']:
        if dependency.get('path'):
            assert Path(dependency['path']).is_relative_to(root), 'private path dependency'
print('MapKit architecture: PASS')
