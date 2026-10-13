"""Exercise release classification and immutable retries without GitHub writes."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest


class ReleasePublishTests(unittest.TestCase):
    def run_publish(self, tag, existing=None):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            scripts = root / 'scripts/release'
            scripts.mkdir(parents=True)
            source = Path(__file__).with_name('publish.sh')
            (scripts / 'publish.sh').write_text(source.read_text())
            (scripts / 'check-version.py').write_text('')
            assets = root / 'release-assets'
            assets.mkdir()
            sha = 'a' * 40
            checksums = []
            for name, data in [('test.deb', b'deb'), ('test.rpm', b'rpm'),
                               ('test.pkg.tar.zst', b'arch'), ('SOURCE_COMMIT', (sha + '\n').encode())]:
                (assets / name).write_bytes(data)
                checksums.append(f'{hashlib.sha256(data).hexdigest()}  {name}\n')
            (assets / 'SHA256SUMS').write_text(''.join(checksums))
            bin_dir = root / 'bin'
            bin_dir.mkdir()
            gh = bin_dir / 'gh'
            gh.write_text('''#!/usr/bin/env python3
import json, os, sys
with open(os.environ['CALLS'], 'a') as log:
    log.write(json.dumps(sys.argv[1:]) + '\\n')
if sys.argv[1] == 'api':
    print(json.dumps([json.loads(os.environ['EXISTING'])]))
''')
            gh.chmod(0o755)
            git = bin_dir / 'git'
            git.write_text('#!/bin/sh\nprintf "%s\\trefs/tags/%s\\n" "$GITHUB_SHA" "$GITHUB_REF_NAME"\n')
            git.chmod(0o755)
            calls = root / 'calls.jsonl'
            env = {**os.environ, 'PATH': f'{bin_dir}:{os.environ["PATH"]}',
                   'GITHUB_SHA': sha, 'GITHUB_REF_NAME': tag,
                   'GITHUB_REPOSITORY': 'test/repo', 'CALLS': str(calls),
                   'EXISTING': json.dumps([] if existing is None else [existing])}
            result = subprocess.run(['bash', str(scripts / 'publish.sh')], cwd=root,
                                    env=env, capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            return [json.loads(line) for line in calls.read_text().splitlines()]

    def test_plain_version_is_normal_release_and_latest(self):
        calls = self.run_publish('v0.1.1')
        create = next(c for c in calls if c[:2] == ['release', 'create'])
        upload = next(c for c in calls if c[:2] == ['release', 'upload'])
        edit = next(c for c in calls if c[:2] == ['release', 'edit'])
        self.assertIn('--prerelease=false', create)
        self.assertIn('--prerelease=false', edit)
        self.assertIn('--latest=true', edit)
        self.assertLess(calls.index(upload), calls.index(edit))

    def test_historical_alpha_remains_prerelease(self):
        calls = self.run_publish('v0.1.0-alpha.1')
        edit = next(c for c in calls if c[:2] == ['release', 'edit'])
        self.assertIn('--prerelease=true', edit)
        self.assertIn('--latest=false', edit)

    def test_published_release_is_not_modified(self):
        calls = self.run_publish('v0.1.1', {'tag_name': 'v0.1.1', 'draft': False})
        self.assertEqual(len(calls), 1)
        self.assertEqual(calls[0][0], 'api')

    def test_existing_draft_is_uploaded_and_published(self):
        calls = self.run_publish('v0.1.1', {'tag_name': 'v0.1.1', 'draft': True})
        self.assertFalse(any(c[:2] == ['release', 'create'] for c in calls))
        self.assertTrue(any(c[:2] == ['release', 'upload'] for c in calls))
        self.assertIn('--draft=false', calls[-1])
