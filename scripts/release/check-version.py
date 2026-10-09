#!/usr/bin/env python3
"""Reject divergent release metadata before building or tagging."""
import json
import pathlib
import re
import sys
import tomllib

root = pathlib.Path(__file__).resolve().parents[2]
version = json.loads((root / 'package.json').read_text())['version']
assert re.fullmatch(r'\d+\.\d+\.\d+(?:-alpha(?:\.\d+)?)?', version), version
for path in ['src-tauri/tauri.conf.json', '.release-please-manifest.json']:
    data = json.loads((root / path).read_text())
    assert data.get('version', data.get('.')) == version, path
if '--sync-lock' in sys.argv:
    path = root / 'src-tauri/Cargo.lock'
    content, count = re.subn(
        r'(name = "mc-gui-crafter"\nversion = ")[^"]+(")',
        lambda match: match[1] + version + match[2], path.read_text(),
    )
    assert count == 1, 'Expected exactly one application in Cargo.lock'
    path.write_text(content)
    sys.argv.remove('--sync-lock')
cargo = tomllib.loads((root / 'src-tauri/Cargo.toml').read_text())
lock = tomllib.loads((root / 'src-tauri/Cargo.lock').read_text())
assert cargo['package']['version'] == version, 'Cargo.toml'
assert next(p['version'] for p in lock['package'] if p['name'] == cargo['package']['name']) == version, 'Cargo.lock'
if len(sys.argv) > 1:
    assert sys.argv[1] == f'v{version}', 'Tag does not match release version'
print(version)
