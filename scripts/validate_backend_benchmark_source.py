#!/usr/bin/env python3
"""Allow HTTP ownership graph comparisons without changing profiles or lock versions."""
import json
import subprocess
import sys
import tomllib


def git(*args):
    return subprocess.check_output(['git', *args], text=True)


def document(revision, path):
    return tomllib.loads(git('show', f'{revision}:{path}'))


def main():
    baseline, candidate = [git("rev-parse", "--verify", "--end-of-options", revision + "^{commit}").strip()
                           for revision in sys.argv[1:]]
    profiles = []
    for revision in [baseline, candidate]:
        paths = git('ls-tree', '-r', '--name-only', revision, '--', 'backend-school').splitlines()
        current = {}
        for path in paths:
            if path.endswith('/Cargo.toml'):
                data = document(revision, path)
                owner = data.get('package', {}).get('name', 'workspace')
                profile = data.get('profile', {})
                if profile:
                    current[owner] = profile
        profiles.append(current)
    if profiles[0] != profiles[1]:
        raise ValueError('Compiler profile changes are outside the HTTP ownership benchmark')
    locks = []
    for revision in [baseline, candidate]:
        data = document(revision, 'backend-school/Cargo.lock')
        locks.append({(p['name'], p['version'], p.get('source'), p.get('checksum'))
                      for p in data.get('package', []) if p.get('source')})
    if locks[0] != locks[1]:
        raise ValueError('External dependency versions/checksums changed')
    print(json.dumps({'profilesUnchanged': True, 'externalLockIdentitiesUnchanged': True,
                      'externalPackages': len(locks[0])}))


if __name__ == '__main__':
    main()
