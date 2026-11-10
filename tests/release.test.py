"""Release guard tests; CI downloads and publication are always mocked."""
import base64
import contextlib
import hashlib
import importlib.util
import io
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

ROOT = Path(__file__).resolve().parent.parent


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    assert spec is not None and spec.loader is not None, path
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


release = load('release', ROOT / 'scripts/build.py')
notify = load('notify', ROOT / 'scripts/notify_discord.py')
SHA = 'a' * 40


class NotificationTests(unittest.TestCase):
    def test_historical_installer_links_keep_the_published_asset_name(self):
        self.assertEqual(notify.installer_asset("0.3.2"), "felaris-setup.exe")
        self.assertEqual(notify.installer_asset("v0.2.3"), "felaris-setup.exe")
        self.assertEqual(notify.installer_asset("0.3.3"), "veluno-setup.exe")
        self.assertEqual(notify.installer_asset("1.2.3"), "veluno-setup.exe")

    def test_payload_matches_delivery_and_normalizes_bullets(self):
        with tempfile.TemporaryDirectory() as directory:
            binary = Path(directory) / 'veluno-setup.exe'
            binary.write_bytes(b'fixture')
            for option, upload in (('--installer', True), ('--exe', True), ('--installer', False)):
                arguments = ['notify', '--version', '1.2.3', '--summary', '* Fixed hover\n- Added prompts', option, str(binary)]
                if not upload:
                    arguments.append('--no-upload')
                with patch('sys.argv', arguments), patch.object(notify, 'get_webhook_url', return_value='https://example.test/webhook'), \
                        patch.object(notify, 'send_webhook', return_value={'id': '123'}) as send, \
                        contextlib.redirect_stdout(io.StringIO()):
                    notify.main()
                payload = send.call_args.args[1]
                self.assertEqual(payload['allowed_mentions'], {'parse': []})
                embed = payload['embeds'][0]
                self.assertEqual(embed['color'], 4363994)
                self.assertEqual(embed['title'], 'Release: Veluno v1.2.3')
                self.assertEqual(embed['fields'][-1]['value'], '- Fixed hover\n- Added prompts')
                self.assertEqual(embed['fields'][2]['name'], 'Attached File' if upload else 'Download')
                self.assertEqual(send.call_args.kwargs['file_path'], binary if upload else None)
                self.assertIn('attached' if upload else 'link below', payload['content'])

    def test_default_delivery_uses_installer(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            installer = root / 'target/release/veluno-setup.exe'
            installer.parent.mkdir(parents=True)
            (installer.parent / 'veluno.exe').write_bytes(b'raw executable')
            for mode in ('attached', 'oversized', 'missing', 'no-upload'):
                with self.subTest(mode=mode):
                    installer.write_bytes(b'installer')
                    if mode == 'oversized':
                        with installer.open('r+b') as artifact:
                            artifact.truncate(10 * 1024 * 1024 + 1)
                    elif mode == 'missing':
                        installer.unlink()
                    arguments = ['notify', '--version', '1.2.3', '--summary', 'Fixed release']
                    if mode == 'no-upload':
                        arguments.append('--no-upload')
                    with patch('sys.argv', arguments), patch.object(notify, 'PROJECT_ROOT', root), \
                            patch.object(notify, 'get_webhook_url', return_value='https://example.test/webhook'), \
                            patch.object(notify, 'urlopen', return_value=io.BytesIO(b'{"id":"123"}')) as post, \
                            contextlib.redirect_stdout(io.StringIO()):
                        notify.main()
                    request = post.call_args.args[0]
                    if mode == 'attached':
                        self.assertIn(b'filename="veluno-setup.exe"', request.data)
                        self.assertIn(b'installer', request.data)
                        self.assertNotIn(b'filename="veluno.exe"', request.data)
                    else:
                        payload = json.loads(request.data)
                        self.assertEqual(payload['embeds'][0]['fields'][2]['value'],
                                         '[veluno-setup.exe](https://github.com/khentmoba/Veluno/releases/download/v1.2.3/veluno-setup.exe)')
                    self.assertIn(b'Run veluno-setup.exe', request.data)

    @unittest.skipUnless(sys.platform == 'win32', 'PowerShell entry point is Windows-only')
    def test_powershell_installer_path_and_legacy_alias(self):
        script = str(ROOT / 'scripts/notify-discord.ps1').replace("'", "''")
        for parameter in ('InstallerPath', 'ExePath'):
            command = ("function python { ConvertTo-Json -InputObject @($args) -Compress; $global:LASTEXITCODE = 37 }; "
                       f"& '{script}' -{parameter} 'C:\\release files\\veluno-setup.exe' -DryRun; exit $LASTEXITCODE")
            encoded = base64.b64encode(command.encode('utf-16le')).decode('ascii')
            result = subprocess.run(['powershell', '-NoProfile', '-EncodedCommand', encoded], capture_output=True, text=True)
            self.assertEqual(result.returncode, 37, result.stderr)
            self.assertEqual(json.loads(result.stdout), [str(ROOT / 'scripts/notify_discord.py'), '--dry-run',
                             '--installer', 'C:\\release files\\veluno-setup.exe'])

    def test_webhook_requests_message_receipt(self):
        response = io.BytesIO(b'{"id":"123"}')
        with patch.object(notify, 'urlopen', return_value=response) as post:
            self.assertEqual(notify.send_webhook('https://example.test/webhook?wait=false', {'content': 'test'}), {'id': '123'})
        self.assertEqual(post.call_args.args[0].full_url, 'https://example.test/webhook?wait=true')


class ReleaseTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        (self.root / 'Cargo.toml').write_text('[package]\nversion = "1.2.3"\n', encoding='utf-8')
        self.manifest = self.root / 'version.json'
        self.manifest.write_text(json.dumps({
            'version': '1.2.3',
            'release_url': 'https://github.com/khentmoba/Veluno/releases/tag/v1.2.3',
            'download_url': 'https://github.com/khentmoba/Felaris/releases/download/v1.2.3/felaris.exe',
            'summary': 'Committed notes',
        }), encoding='utf-8')
        self.commands = []
        self.dirty = ''
        self.head = SHA
        self.remote = SHA
        self.tag = ''
        self.ci = [{'databaseId': 42, 'headSha': SHA, 'status': 'completed', 'conclusion': 'success'}]
        self.failing_stage = None
        self.missing_asset = False
        self.corrupt_asset = False

    def run_command(self, command, **kwargs):
        self.commands.append(command)
        result = ''
        if command[:2] == ['git', 'status']:
            result = self.dirty
        elif command[:2] == ['git', 'rev-parse']:
            result = self.head
        elif command[:3] == ['git', 'ls-remote', '--tags']:
            result = self.tag
        elif command[:2] == ['git', 'ls-remote']:
            result = self.remote + '\trefs/heads/main'
        elif command[:3] == ['gh', 'run', 'list']:
            result = json.dumps(self.ci)
        else:
            stage = ('download' if command[:3] == ['gh', 'run', 'download'] else 'github' if command[0] == 'gh'
                     else 'notify' if command[0] == sys.executable else 'other')
            if stage == self.failing_stage:
                if kwargs.get('check'):
                    raise subprocess.CalledProcessError(7, command)
                return SimpleNamespace(returncode=7)
            if stage == 'download':
                self.assertEqual(command[3:6], ['42', '--name', 'Veluno-Windows'])
                directory = Path(command[command.index('--dir') + 1])
                sums = []
                for name in release.ASSETS[:-1]:
                    (directory / name).write_bytes(name.encode())
                    sums.append(f'{hashlib.sha256(name.encode()).hexdigest()}  {name}')
                if not self.missing_asset:
                    (directory / 'SHA256SUMS').write_text('\n'.join(sums) + '\n', encoding='ascii')
                if self.corrupt_asset:
                    (directory / 'veluno-setup.exe').write_bytes(b'tampered')
        return SimpleNamespace(stdout=result, returncode=0)

    def invoke(self, *arguments, gh: str | None = 'gh'):
        with patch.object(release, 'PROJECT_ROOT', self.root), patch('sys.argv', ['build.py', *arguments]), \
                patch('subprocess.run', side_effect=self.run_command), patch('shutil.which', return_value=gh), \
                contextlib.redirect_stdout(io.StringIO()):
            release.main()

    def published(self):
        return [command for command in self.commands if command[:3] == ['gh', 'release', 'create'] or command[0] == sys.executable]

    def test_github_failure_stops_discord(self):
        self.failing_stage = 'github'
        with self.assertRaises(subprocess.CalledProcessError):
            self.invoke()
        self.assertEqual(len(self.published()), 1)

    def test_download_failure_stops_publication(self):
        self.failing_stage = 'download'
        with self.assertRaises(subprocess.CalledProcessError):
            self.invoke()
        self.assertEqual(self.published(), [])

    def test_publishing_preserves_metadata_and_targets_built_commit(self):
        before = self.manifest.read_bytes()
        self.invoke()
        self.assertEqual(self.manifest.read_bytes(), before)
        github, notification = self.published()
        self.assertEqual(github[github.index('--target') + 1], SHA)
        self.assertIn('Committed notes', notification)
        self.assertEqual(github[4:9], [str(self.root / 'target/release' / name)
                                     for name in ('veluno.exe', 'veluno-setup.exe', 'felaris.exe', 'felaris-setup.exe', 'SHA256SUMS')])
        self.assertEqual(json.loads(self.manifest.read_text())['download_url'],
                         release.manifest_for('1.2.3', 'Committed notes')['download_url'])

    def test_prepare_only_writes_metadata(self):
        self.invoke('--prepare', '--summary', 'New notes')
        self.assertEqual(json.loads(self.manifest.read_text())['summary'], 'New notes')
        self.assertEqual(self.commands, [])

    def test_unprepared_summary_rejected_before_build(self):
        with self.assertRaisesRegex(ValueError, '--summary differs'):
            self.invoke('--summary', 'Uncommitted notes')
        self.assertEqual(self.commands, [])

    def test_dirty_checkout_rejected(self):
        self.dirty = ' M version.json'
        with self.assertRaisesRegex(ValueError, 'clean'):
            self.invoke()
        self.assertEqual(self.published(), [])

    def test_unmerged_commit_rejected(self):
        self.remote = 'b' * 40
        with self.assertRaisesRegex(ValueError, 'origin/main'):
            self.invoke()
        self.assertEqual(self.published(), [])

    def test_existing_tag_rejected(self):
        self.tag = SHA + '\trefs/tags/v1.2.3'
        with self.assertRaisesRegex(ValueError, 'already exists'):
            self.invoke()
        self.assertEqual(self.published(), [])

    def test_github_cli_is_required_to_fetch_ci_assets(self):
        for arguments in ((), ('--skip-github',)):
            with self.subTest(arguments=arguments), self.assertRaisesRegex(ValueError, 'required'):
                self.invoke(*arguments, gh=None)
        self.assertEqual(self.published(), [])

    def test_skip_github_still_uses_ci_assets_and_notifies(self):
        self.invoke('--skip-github')
        self.assertEqual(len(self.published()), 1)
        self.assertEqual(self.published()[0][0], sys.executable)
        self.assertTrue(any(command[:3] == ['gh', 'run', 'download'] for command in self.commands))

    def test_checksum_mismatch_rejected(self):
        self.corrupt_asset = True
        with self.assertRaisesRegex(ValueError, 'SHA256SUMS'):
            self.invoke()
        self.assertEqual(self.published(), [])

    def test_missing_verified_asset_rejected(self):
        self.missing_asset = True
        with self.assertRaisesRegex(ValueError, 'Missing verified'):
            self.invoke()
        self.assertEqual(self.published(), [])

    def test_dry_run_never_builds_or_changes_metadata(self):
        before = self.manifest.read_bytes()
        self.invoke('--dry-run')
        self.assertTrue(all(command[0] == 'git' or command[:3] == ['gh', 'run', 'list'] for command in self.commands))
        self.assertEqual(self.manifest.read_bytes(), before)

    def test_mismatched_version_rejected(self):
        (self.root / 'Cargo.toml').write_text('[package]\nversion = "2.0.0"\n', encoding='utf-8')
        with self.assertRaisesRegex(ValueError, 'match Cargo.toml'):
            self.invoke()

    def test_ci_must_pass_on_the_exact_commit(self):
        for runs in ([], [{'headSha': SHA, 'status': 'in_progress', 'conclusion': ''}],
                     [{'databaseId': 42, 'headSha': SHA, 'status': 'completed', 'conclusion': 'failure'}],
                     [{'databaseId': 42, 'headSha': 'b' * 40, 'status': 'completed', 'conclusion': 'success'}]):
            with self.subTest(runs=runs):
                self.ci = runs
                with self.assertRaisesRegex(ValueError, 'CI must pass'):
                    self.invoke()
                self.assertEqual(self.published(), [])

    @unittest.skipUnless(sys.platform == 'win32', 'PowerShell entry point is Windows-only')
    def test_powershell_wrapper_forwards_arguments_and_failure(self):
        script = str(ROOT / 'scripts/build.ps1').replace("'", "''")
        command = ("function python { ConvertTo-Json -InputObject @($args) -Compress; $global:LASTEXITCODE = 37 }; "
                   f"& '{script}' -Summary 'Release notes with spaces' -Prepare -SkipGitHub -SkipDiscordNotify -DryRun; exit $LASTEXITCODE")
        encoded = base64.b64encode(command.encode('utf-16le')).decode('ascii')
        result = subprocess.run(['powershell', '-NoProfile', '-EncodedCommand', encoded], capture_output=True, text=True)
        self.assertEqual(result.returncode, 37, result.stderr)
        self.assertEqual(json.loads(result.stdout), [str(ROOT / 'scripts/build.py'), '--summary',
                         'Release notes with spaces', '--prepare', '--skip-discord', '--skip-github', '--dry-run'])


if __name__ == '__main__':
    unittest.main()
