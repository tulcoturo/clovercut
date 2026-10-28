"""Minimum-OS gate regressions using tool fixtures; no macOS or package install needed."""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parent.parent
TOOL = '''#!/usr/bin/env python3
import json, sys
from pathlib import Path
path=Path(sys.argv[-1])
if Path(sys.argv[0]).name == 'file':
 print('Mach-O 64-bit dynamically linked shared library' if path.suffix=='.macho' else 'ASCII text')
else:
 data=json.loads(path.read_text())
 if data.get('minimum'):
  print('cmd LC_VERSION_MIN_MACOSX\\nversion '+data['minimum'] if data.get('legacy') else 'cmd LC_BUILD_VERSION\\nminos '+data['minimum'])
'''


class MinimumOS(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.bundle = self.root / 'Clovercut.app'
        self.bundle.mkdir()
        self.tools = self.root / 'bin'
        self.tools.mkdir()
        for name in ('file', 'otool'):
            path = self.tools / name
            path.write_text(TOOL)
            path.chmod(0o755)
        self.env = dict(os.environ, PATH=str(self.tools) + os.pathsep + os.environ['PATH'])
        self.env.pop('GITHUB_STEP_SUMMARY', None)
        self.env.pop('GITHUB_ACTIONS', None)

    def binary(self, name, minimum, **fields):
        path = self.bundle / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(dict(minimum=minimum, **fields)))

    def check(self, *args, minimum='12.0'):
        return subprocess.run(['bash', str(ROOT / 'scripts/ci/check-min-os.sh'),
                               str(self.bundle), minimum, *args], env=self.env,
                              text=True, capture_output=True)

    def test_eager_homebrew_library_above_monterey_fails_enforcement(self):
        self.binary('Contents/MacOS/app.macho', '12.0')
        self.binary('Contents/Frameworks/libavfilter.macho', '14.0')
        result = self.check('--enforce')
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('libavfilter.macho needs 14.0', result.stdout)
        # Diagnostic-only invocation remains explicitly non-enforcing.
        self.assertEqual(self.check().returncode, 0)

    def test_equivalent_zero_padded_minimum_is_not_newer(self):
        self.binary('Contents/Frameworks/library with spaces.macho', '12.0.0')
        result = self.check('--enforce')
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_legacy_headers_are_checked_numerically(self):
        self.binary('legacy.macho', '12.10', legacy=True)
        self.assertNotEqual(self.check('--enforce', minimum='12.9').returncode, 0)
        self.assertEqual(self.check('--enforce', minimum='12.11').returncode, 0)

    def test_unknown_minimum_cannot_qualify_a_macho_file(self):
        self.binary('unknown.macho', None)
        self.assertNotEqual(self.check('--enforce').returncode, 0)

    def test_only_explicit_lazy_runtime_keeps_its_separate_floor(self):
        self.binary('app.macho', '12.0')
        self.binary('runtime.macho', '13.4')
        self.assertEqual(self.check('--enforce', '--lazy', 'runtime.macho').returncode, 0)
        self.binary('unrelated.macho', '14.0')
        self.assertNotEqual(self.check('--enforce', '--lazy', 'runtime.macho').returncode, 0)

    def test_missing_or_unversioned_lazy_runtime_fails(self):
        self.binary('app.macho', '12.0')
        self.assertNotEqual(self.check('--enforce', '--lazy', 'missing.macho').returncode, 0)
        self.binary('runtime.macho', None)
        self.assertNotEqual(self.check('--enforce', '--lazy', 'runtime.macho').returncode, 0)

    def test_no_macho_files_is_not_a_qualified_bundle(self):
        self.assertNotEqual(self.check('--enforce').returncode, 0)
