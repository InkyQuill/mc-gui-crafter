"""Exercise release metadata drift and narrow lock synchronization in a fixture."""
import json
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest


class ReleaseVersionTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        source = Path(__file__).resolve().parents[2]
        for name in ['package.json', 'src-tauri/tauri.conf.json',
                     'src-tauri/Cargo.toml', 'src-tauri/Cargo.lock',
                     '.release-please-manifest.json', 'scripts/release/check-version.py']:
            dest = self.root / name
            dest.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(source / name, dest)
        self.version = json.loads((self.root / 'package.json').read_text())['version']

    def run_check(self, *args):
        return subprocess.run(['python3', str(self.root / 'scripts/release/check-version.py'),
                               *args], capture_output=True, text=True)

    def test_tag_must_match_version(self):
        self.assertEqual(self.run_check('v' + self.version).returncode, 0)
        self.assertNotEqual(self.run_check('v999.0.0').returncode, 0)

    def test_drift_in_each_manifest_is_rejected(self):
        for name in ['package.json', 'src-tauri/tauri.conf.json',
                     'src-tauri/Cargo.toml', 'src-tauri/Cargo.lock',
                     '.release-please-manifest.json']:
            with self.subTest(name=name):
                path = self.root / name
                before = path.read_text()
                path.write_text(before.replace(self.version, '999.0.0'))
                self.assertNotEqual(self.run_check().returncode, 0)
                path.write_text(before)

    def test_lock_sync_changes_only_application_version(self):
        path = self.root / 'src-tauri/Cargo.lock'
        before = path.read_text()
        path.write_text(before.replace('name = "mc-gui-crafter"\nversion = "' + self.version + '"',
                                       'name = "mc-gui-crafter"\nversion = "0.0.0"'))
        self.assertNotEqual(self.run_check().returncode, 0)
        self.assertEqual(self.run_check('--sync-lock').returncode, 0)
        self.assertEqual(path.read_text(), before)
