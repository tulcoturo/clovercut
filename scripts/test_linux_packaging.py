"""Fast packaging/installer regressions; never install anything on the host."""
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parent.parent
MOCK = r'''#!/usr/bin/env python3
import hashlib,json,os,sys
from pathlib import Path
name=Path(sys.argv[0]).name;args=sys.argv[1:]
with open(os.environ['CALLS'],'a') as f:f.write(json.dumps([name,*args])+'\n')
if name=='uname':print(os.environ.get('ARCH','x86_64') if args==['-m'] else 'Linux')
elif name=='id':print('1000')
elif name=='curl':
 if os.environ.get('NETWORK_FAIL')=='1':sys.exit(22)
 url=args[-1] if '-o' not in args else next(a for a in args if a.startswith('https://'))
 ver=os.environ.get('RELEASE','0.1.8');arch=os.environ.get('ARCH','x86_64');arch='aarch64' if arch=='arm64' else arch
 data=b'checked app payload\n'
 if url.endswith('/latest'):out=json.dumps({'tag_name':'v'+ver}).encode()
 elif url.endswith('/SHA256SUMS'):
  if os.environ.get('EMPTY_SUMS')=='1':out=b''
  elif os.environ.get('SUMS_FILE'):out=Path(os.environ['SUMS_FILE']).read_bytes()
  else:
   kinds=['AppImage']+(['deb','rpm'] if os.environ.get('NATIVE')=='1' else [])
   out=''.join(hashlib.sha256(data).hexdigest()+'  Clovercut-'+ver+'-'+arch+'.'+k+'\n' for k in kinds).encode()
 elif url.startswith('https://aur.archlinux.org/rpc/'):
  if os.environ.get('AUR_FAIL')=='1':sys.exit(22)
  out=json.dumps({'results':[{'Version':os.environ.get('AUR_RELEASE',ver+'-1')}]}).encode()
 else:out=b'corrupt\n' if os.environ.get('CORRUPT')=='1' else data
 if '-o' in args:Path(args[args.index('-o')+1]).write_bytes(out)
 else:sys.stdout.buffer.write(out)
'''


class Installer(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.r = Path(self.tmp.name)
        self.mock = self.r / 'bin'
        self.mock.mkdir()
        for name in ['curl', 'uname', 'id', 'sudo', 'apt-get', 'dnf', 'zypper', 'yay', 'paru']:
            p = self.mock / name
            p.write_text(MOCK)
            p.chmod(0o755)
        self.os_release = self.r / 'os-release'
        # Replace only the OS metadata path to exercise distributions independently
        # of the machine running the tests. Package managers/network are mocked.
        self.script = self.r / 'install.sh'
        self.script.write_text((ROOT / 'scripts/install.sh').read_text().replace('/etc/os-release', str(self.os_release)))
        self.env = {**os.environ, 'PATH': str(self.mock)+os.pathsep+os.environ['PATH'],
                    'HOME': str(self.r / 'home'), 'CALLS': str(self.r / 'calls')}
        for key in ['VERSION', 'PREFIX', 'RELEASE', 'ARCH', 'NATIVE', 'CORRUPT', 'EMPTY_SUMS', 'NETWORK_FAIL', 'AUR_RELEASE', 'AUR_FAIL']:
            self.env.pop(key, None)

    def run_install(self, distro='debian', **env):
        self.os_release.write_text('ID='+distro+'\n')
        self.env.update(env)
        return subprocess.run(['sh', str(self.script)], env=self.env, text=True, capture_output=True)

    def calls(self):
        return [json.loads(line) for line in (self.r / 'calls').read_text().splitlines()]

    def test_debian_old_release_falls_back_to_verified_appimage(self):
        result = self.run_install()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual((self.r / 'home/.local/bin/clovercut').read_bytes(), b'checked app payload\n')
        self.assertFalse(any(c[0] == 'sudo' for c in self.calls()))

    def test_fedora_old_release_also_falls_back(self):
        result = self.run_install('fedora')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue((self.r / 'home/.local/bin/clovercut').is_file())

    def test_native_deb_is_verified_before_package_manager(self):
        result = self.run_install(RELEASE='0.1.9', NATIVE='1')
        self.assertEqual(result.returncode, 0, result.stderr)
        sudo = next(c for c in self.calls() if c[0] == 'sudo')
        self.assertEqual(sudo[1:4], ['apt-get', 'install', '-y'])
        self.assertTrue(sudo[-1].endswith('.deb'))
        self.assertFalse((self.r / 'home/.local/bin/clovercut').exists())

    def test_native_install_reports_existing_launcher_that_shadows_package(self):
        launcher = self.mock / 'clovercut'
        original = b'#!/bin/sh\necho previous portable installation\n'
        launcher.write_bytes(original)
        launcher.chmod(0o755)
        result = self.run_install(RELEASE='0.1.9', NATIVE='1')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(launcher.read_bytes(), original)
        self.assertIn(str(launcher), result.stdout)
        self.assertIn('/usr/bin/clovercut', result.stdout)

    def test_opensuse_uses_zypper(self):
        (self.mock / 'dnf').unlink()
        result = self.run_install('opensuse', RELEASE='0.1.9', NATIVE='1')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue(any(c[:3] == ['sudo', 'zypper', '--non-interactive'] for c in self.calls()))

    def test_explicit_version_on_arch_does_not_ignore_pin(self):
        result = self.run_install('arch', VERSION='0.1.8')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertFalse(any(c[0] in ['yay', 'paru'] for c in self.calls()))
        self.assertTrue((self.r / 'home/.local/bin/clovercut').is_file())

    def test_default_arch_uses_aur_helper(self):
        result = self.run_install('arch')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn(['yay', '-S', 'clovercut-bin'], self.calls())

    def test_arch_stale_aur_uses_the_current_verified_appimage(self):
        result = self.run_install('arch', RELEASE='0.1.9', AUR_RELEASE='0.1.8-1')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertFalse(any(c[0] in ['yay', 'paru'] for c in self.calls()))
        self.assertEqual((self.r / 'home/.local/bin/clovercut').read_bytes(), b'checked app payload\n')

    def test_arch_unavailable_aur_does_not_block_portable_install(self):
        result = self.run_install('arch', RELEASE='0.1.9', AUR_FAIL='1')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertFalse(any(c[0] in ['yay', 'paru'] for c in self.calls()))
        self.assertEqual((self.r / 'home/.local/bin/clovercut').read_bytes(), b'checked app payload\n')

    def test_prefix_requests_portable_install_even_with_native_asset(self):
        result = self.run_install(NATIVE='1', PREFIX=str(self.r / 'custom'))
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue((self.r / 'custom/bin/clovercut').is_file())
        self.assertFalse(any(c[0] == 'sudo' for c in self.calls()))

    def test_arm64_selects_aarch64_asset(self):
        result = self.run_install(ARCH='arm64')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue(any('aarch64.AppImage' in ' '.join(c) for c in self.calls()))

    def test_corrupt_download_preserves_existing_installation(self):
        target = self.r / 'home/.local/bin/clovercut'
        target.parent.mkdir(parents=True)
        target.write_bytes(b'old executable')
        result = self.run_install(CORRUPT='1')
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(target.read_bytes(), b'old executable')
        self.assertFalse(any(c[0] == 'sudo' for c in self.calls()))

    def test_missing_checksums_fail_without_installing(self):
        result = self.run_install(EMPTY_SUMS='1')
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse((self.r / 'home/.local/bin/clovercut').exists())

    def test_invalid_version_is_rejected(self):
        result = self.run_install(VERSION='../../main')
        self.assertNotEqual(result.returncode, 0)

    def test_corrupt_native_download_never_reaches_package_manager(self):
        result = self.run_install(RELEASE='0.1.9', NATIVE='1', CORRUPT='1')
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(any(c[0] == 'sudo' for c in self.calls()))


class NativePackages(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.r = Path(self.tmp.name)
        self.stage = self.r / 'stage'
        (self.stage / 'lib').mkdir(parents=True)
        shutil.copy('/bin/true', self.stage / 'clovercut')
        shutil.copy(ROOT / 'assets/icons/clovercut_256.png', self.stage / 'clovercut.png')
        for name in ['LICENSE', 'THIRD_PARTY_NOTICES.md']:
            shutil.copy(ROOT / name, self.stage / name)
        shutil.copy(ROOT / 'assets/linux/io.github.tulcoturo.clovercut.desktop', self.stage)
        (self.stage / 'lib/libfixture.so').write_bytes(b'fixture')
        (self.stage / 'lib/libfixture-link.so').symlink_to('libfixture.so')

    def command(self, *args):
        if args[0] == 'rpm':
            db = self.r / 'rpm-query-db'
            db.mkdir(exist_ok=True)
            args = ('rpm', '--dbpath', str(db), *args[1:])
        return subprocess.check_output(args, text=True, stderr=subprocess.STDOUT)

    @unittest.skipUnless(shutil.which('dpkg-deb'), 'dpkg-deb required')
    def test_real_deb_has_payload_architecture_and_root_ownership(self):
        out = self.r / 'arm64-in-name.deb'
        self.command('bash', str(ROOT / 'scripts/build-deb.sh'), str(self.stage), '0.1.8', str(out))
        arch = 'arm64' if os.uname().machine in ['aarch64', 'arm64'] else 'amd64'
        self.assertEqual(self.command('dpkg-deb', '-f', str(out), 'Architecture').strip(), arch)
        listing = self.command('dpkg-deb', '-c', str(out))
        self.assertTrue(listing.splitlines())
        for line in listing.splitlines():
            self.assertEqual(line.split()[1], 'root/root')
        unpack = self.r / 'unpack'
        self.command('dpkg-deb', '-x', str(out), str(unpack))
        self.assertEqual((unpack / 'opt/clovercut/clovercut').read_bytes(), (self.stage / 'clovercut').read_bytes())
        self.assertTrue((unpack / 'opt/clovercut/lib/libfixture-link.so').is_symlink())
        self.assertIn('"$@"', (unpack / 'usr/bin/clovercut').read_text())

    @unittest.skipUnless(shutil.which('rpmbuild'), 'rpmbuild required')
    def test_real_rpm_keeps_payload_and_does_not_publish_private_libraries(self):
        out = self.r / 'test.rpm'
        self.command('bash', str(ROOT / 'scripts/build-rpm.sh'), str(self.stage), '0.1.8', str(out))
        self.assertEqual(self.command('rpm', '-qp', '--qf', '%{ARCH}', str(out)), os.uname().machine)
        listing = self.command('rpm', '-qpl', str(out))
        self.assertIn('/opt/clovercut/lib/libfixture.so', listing)
        self.assertIn('/usr/bin/clovercut', listing)
        self.assertNotIn('libfixture', self.command('rpm', '-qp', '--provides', str(out)))
        rows = self.command('rpm', '-qp', '--qf', '[%{FILENAMES}\t%{FILEDIGESTS}\n]', str(out))
        digest = next(line.split('\t')[1] for line in rows.splitlines() if line.startswith('/opt/clovercut/clovercut\t'))
        self.assertEqual(digest, hashlib.sha256((self.stage / 'clovercut').read_bytes()).hexdigest())

    def test_manifest_includes_all_four_native_linux_packages(self):
        spec = importlib.util.spec_from_file_location('models', ROOT / 'scripts/models.py')
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        for arch in ['x86_64', 'aarch64']:
            for kind in ['AppImage', 'deb', 'rpm']:
                (self.r / f'Clovercut-0.1.9-{arch}.{kind}').write_bytes(b'package')
        manifest = module.release_manifest('0.1.9', 'v0.1.9', self.r)
        for arch in ['x86_64', 'aarch64']:
            self.assertEqual(set(manifest['binaries']['linux'][arch]), {'appimage', 'deb', 'rpm'})


class Aur(unittest.TestCase):
    @unittest.skipUnless(shutil.which('makepkg'), 'makepkg required')
    def test_srcinfo_matches_package_definition(self):
        actual = subprocess.check_output(['makepkg', '--printsrcinfo'], cwd=ROOT / 'assets/arch', text=True)
        self.assertEqual(actual, (ROOT / 'assets/arch/.SRCINFO').read_text())


class AurBump(unittest.TestCase):
    def setUp(self):
        import re
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.r = Path(self.tmp.name)
        (self.r / 'scripts').mkdir()
        (self.r / 'assets/arch').mkdir(parents=True)
        self.bin = self.r / 'bin'
        self.bin.mkdir()
        for name in ['PKGBUILD', '.SRCINFO']:
            shutil.copy(ROOT / 'assets/arch' / name, self.r / 'assets/arch' / name)
        shutil.copy(ROOT / 'scripts/bump-aur.sh', self.r / 'scripts')
        curl = self.bin / 'curl'
        curl.write_text(MOCK)
        curl.chmod(0o755)
        self.env = {**os.environ, 'PATH': str(self.bin)+os.pathsep+os.environ['PATH'],
                    'CALLS': str(self.r / 'calls'), 'SUMS_FILE': str(self.r / 'sums')}
        pkg = (ROOT / 'assets/arch/PKGBUILD').read_text()
        self.version = re.search(r'^pkgver=(.+)$', pkg, re.M).group(1)
        major, minor, patch = map(int, self.version.split('.'))
        self.next_version = f'{major}.{minor}.{patch + 1}'
        lines = []
        for arch in ['x86_64', 'aarch64']:
            value = re.search(r"sha256sums_"+arch+r"=\('([^']+)'\)", pkg).group(1)
            lines.append(value+'  Clovercut-'+self.version+'-'+arch+'.AppImage')
        (self.r / 'sums').write_text('\n'.join(lines)+'\n')

    def bump(self, *args):
        return subprocess.run(['bash', str(self.r / 'scripts/bump-aur.sh'), *args], env=self.env, capture_output=True, text=True)

    @unittest.skipUnless(shutil.which('makepkg'), 'makepkg required')
    def test_check_rejects_stale_srcinfo(self):
        result = self.bump(self.version, '--check')
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        info = self.r / 'assets/arch/.SRCINFO'
        stale = '0.0.0' if self.version != '0.0.0' else '0.0.1'
        info.write_text(info.read_text().replace(f'pkgver = {self.version}', f'pkgver = {stale}'))
        self.assertNotEqual(self.bump(self.version, '--check').returncode, 0)
        self.assertIn(f'pkgver = {stale}', info.read_text())

    @unittest.skipUnless(shutil.which('makepkg'), 'makepkg required')
    def test_check_rejects_unreachable_checksums(self):
        self.env['NETWORK_FAIL'] = '1'
        self.assertNotEqual(self.bump(self.version, '--check').returncode, 0)

    @unittest.skipUnless(shutil.which('makepkg'), 'makepkg required')
    def test_failed_generation_preserves_both_original_files(self):
        old = {n: (self.r / 'assets/arch' / n).read_bytes() for n in ['PKGBUILD', '.SRCINFO']}
        (self.r / 'sums').write_text('a'*64+f'  Clovercut-{self.next_version}-x86_64.AppImage\n'+'b'*64+f'  Clovercut-{self.next_version}-aarch64.AppImage\n')
        tool = self.bin / 'makepkg'
        tool.write_text('#!/bin/sh\nexit 1\n')
        tool.chmod(0o755)
        self.assertNotEqual(self.bump(self.next_version).returncode, 0)
        for name, value in old.items():
            self.assertEqual((self.r / 'assets/arch' / name).read_bytes(), value)


if __name__ == '__main__':
    unittest.main()
