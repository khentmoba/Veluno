#!/usr/bin/env python3
"""Prepare release metadata, or publish the installer CI built for the committed release."""
import argparse
import hashlib
import json
import shutil
import subprocess
import sys
import tomllib
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parent.parent
ASSETS = ('veluno.exe', 'veluno-setup.exe', 'felaris.exe', 'felaris-setup.exe', 'SHA256SUMS')
CI_ARTIFACT = 'Veluno-Windows'


def git(*arguments):
    return subprocess.run(['git', *arguments], cwd=PROJECT_ROOT, check=True,
                          capture_output=True, text=True).stdout.strip()


def manifest_for(version, summary):
    return {
        'version': version,
        'release_url': f'https://github.com/khentmoba/Veluno/releases/tag/v{version}',
        'download_url': f'https://github.com/khentmoba/Felaris/releases/download/v{version}/felaris.exe',
        'summary': summary,
    }


def clean_commit():
    if git('status', '--porcelain'):
        raise ValueError('Release checkout must be clean. Prepare and commit version.json before publishing.')
    return git('rev-parse', 'HEAD')


def verify_checksums(directory):
    expected = {}
    for line in (directory / 'SHA256SUMS').read_text(encoding='ascii').splitlines():
        digest, _, name = line.partition('  ')
        expected[name] = digest
    for name in ASSETS[:-1]:
        actual = hashlib.sha256((directory / name).read_bytes()).hexdigest()
        if expected.get(name) != actual:
            raise ValueError(f'{name} does not match SHA256SUMS from CI')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--prepare', action='store_true', help='Write version.json only; commit it before publishing')
    parser.add_argument('--skip-discord', action='store_true')
    parser.add_argument('--skip-github', action='store_true')
    parser.add_argument('--summary', '-s', help='Required for preparation; otherwise must match committed metadata')
    parser.add_argument('--dry-run', action='store_true', help='Validate and print the plan without building or publishing')
    args = parser.parse_args()
    version = tomllib.loads((PROJECT_ROOT / 'Cargo.toml').read_text(encoding='utf-8'))['package']['version']
    manifest_path = PROJECT_ROOT / 'version.json'
    if args.prepare:
        if not args.summary or not args.summary.strip():
            parser.error('--prepare requires a nonempty --summary')
        if args.dry_run:
            parser.error('--prepare and --dry-run are separate operations')
        manifest_path.write_text(json.dumps(manifest_for(version, args.summary), indent=2) + '\n', encoding='utf-8')
        print('Prepared version.json. Commit and push the release metadata directly to main before publishing.')
        return

    manifest = json.loads(manifest_path.read_text(encoding='utf-8-sig'))
    summary = manifest.get('summary')
    if not isinstance(summary, str) or not summary.strip() or manifest != manifest_for(version, summary):
        raise ValueError('version.json must match Cargo.toml and contain release notes. Run --prepare first.')
    if args.summary is not None and args.summary != summary:
        raise ValueError('--summary differs from committed release notes. Run --prepare and commit the change first.')
    commit = clean_commit()
    if not shutil.which('gh'):
        raise ValueError("GitHub CLI 'gh' is required to fetch the CI-built installer.")
    remote_main = git('ls-remote', 'origin', 'refs/heads/main').split()
    if not remote_main or remote_main[0] != commit:
        raise ValueError('Publish from the exact commit on origin/main, after its checks pass.')
    result = subprocess.run(['gh', 'run', 'list', '--workflow', 'ci.yml', '--event', 'push',
                             '--branch', 'main', '--commit', commit, '--limit', '1',
                             '--json', 'databaseId,headSha,status,conclusion'],
                            cwd=PROJECT_ROOT, check=True, capture_output=True, text=True)
    runs = json.loads(result.stdout)
    run = runs[0] if runs else {}
    passed = {'headSha': commit, 'status': 'completed', 'conclusion': 'success'}
    if {key: run.get(key) for key in passed} != passed:
        raise ValueError('Windows CI must pass on the exact release commit before publication.')
    if not args.skip_github and git('ls-remote', '--tags', 'origin', f'refs/tags/v{version}', f'refs/tags/v{version}^{{}}'):
        raise ValueError(f'Tag v{version} already exists. Prepare a new version before publishing.')
    print(f'Veluno v{version}, source commit {commit}, CI run {run["databaseId"]}')
    if args.dry_run:
        print(f'Would download {CI_ARTIFACT} from CI; GitHub: {not args.skip_github}; Discord: {not args.skip_discord}')
        return

    release = PROJECT_ROOT / 'target/release'
    release.mkdir(parents=True, exist_ok=True)
    for name in ASSETS:
        (release / name).unlink(missing_ok=True)
    subprocess.run(['gh', 'run', 'download', str(run['databaseId']), '--name', CI_ARTIFACT, '--dir', str(release)],
                   cwd=PROJECT_ROOT, check=True)
    assets = [release / name for name in ASSETS]
    for asset in assets:
        if not asset.is_file():
            raise ValueError(f'Missing verified release asset: {asset}')
    verify_checksums(release)
    if not args.skip_github:
        subprocess.run(['gh', 'release', 'create', f'v{version}', *map(str, assets),
                        '--target', commit, '--title', f'Veluno v{version}', '--notes', summary],
                       cwd=PROJECT_ROOT, check=True)
    if not args.skip_discord:
        subprocess.run([sys.executable, str(PROJECT_ROOT / 'scripts/notify_discord.py'), '--summary', summary],
                       cwd=PROJECT_ROOT, check=True)


if __name__ == '__main__':
    try:
        main()
    except (ValueError, OSError, subprocess.CalledProcessError) as error:
        print(f'Release failed: {error}', file=sys.stderr)
        sys.exit(1)
