#!/usr/bin/env python3
"""Create a VAPID signing key for the personal backend's Web Push channel."""
import argparse
import base64
import os
from pathlib import Path
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
DATA = ROOT / '.data'
KEY = DATA / 'vapid.pem'
ENV = DATA / 'server.env'


def env_value(value: str) -> str:
    return '"' + value.replace('\\', '\\\\').replace('"', '\\"') + '"'


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--subject', required=True, help='VAPID contact, such as mailto:you@example.com or an HTTPS URL')
    args = parser.parse_args()
    subject = args.subject.strip()
    if not (subject.startswith('mailto:') and re.fullmatch(r'mailto:[^@\s]+@[^@\s]+', subject) or subject.startswith('https://')):
        parser.error('--subject must be a mailto address or HTTPS URL')
    if not shutil_which('openssl'):
        print('OpenSSL is required.', file=sys.stderr)
        return 2

    DATA.mkdir(mode=0o700, parents=True, exist_ok=True)
    os.chmod(DATA, 0o700)
    if KEY.exists():
        print(f'{KEY} already exists; refusing to overwrite the signing key.', file=sys.stderr)
        return 2
    existing = ENV.read_text(encoding='utf-8') if ENV.exists() else ''
    if any(line.partition('=')[0] in {'NHIP_VAPID_PEM', 'NHIP_VAPID_PUBLIC', 'NHIP_VAPID_SUBJECT'} for line in existing.splitlines() if '=' in line):
        print(f'VAPID settings already exist in {ENV}; refusing to replace them.', file=sys.stderr)
        return 2

    fd = os.open(KEY, os.O_CREAT | os.O_EXCL | os.O_WRONLY, 0o600)
    try:
        with os.fdopen(fd, 'wb') as output:
            subprocess.run(['openssl', 'ecparam', '-name', 'prime256v1', '-genkey', '-noout'], stdout=output, check=True)
            output.flush()
            os.fsync(output.fileno())
        der = subprocess.check_output(['openssl', 'ec', '-in', str(KEY), '-pubout', '-outform', 'DER'], stderr=subprocess.DEVNULL)
        public_point = der[-65:]
        if len(public_point) != 65 or public_point[0] != 4:
            raise RuntimeError('OpenSSL returned an invalid P-256 public key.')
        public_key = base64.urlsafe_b64encode(public_point).decode('ascii').rstrip('=')
        additions = [
            f'NHIP_VAPID_PEM={env_value(str(KEY))}',
            f'NHIP_VAPID_PUBLIC={env_value(public_key)}',
            f'NHIP_VAPID_SUBJECT={env_value(subject)}',
        ]
        with ENV.open('a', encoding='utf-8') as settings:
            if existing and not existing.endswith('\n'):
                settings.write('\n')
            settings.write('\n'.join(additions) + '\n')
        os.chmod(ENV, 0o600)
    except Exception:
        KEY.unlink(missing_ok=True)
        raise

    print(f'Created private VAPID key at {KEY} (mode 0600).')
    print(f'Added Web Push settings to {ENV} (mode 0600).')
    print('Restart the backend to enable Web Push. The private key must stay on this server.')
    return 0


def shutil_which(command: str) -> str | None:
    for directory in os.environ.get('PATH', '').split(os.pathsep):
        candidate = Path(directory) / command
        if candidate.is_file() and os.access(candidate, os.X_OK):
            return str(candidate)
    return None


if __name__ == '__main__':
    try:
        raise SystemExit(main())
    except (OSError, subprocess.CalledProcessError, RuntimeError) as exc:
        print(f'Could not create VAPID configuration: {exc}', file=sys.stderr)
        raise SystemExit(1)
