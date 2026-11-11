#!/usr/bin/env python3
"""Install or remove Nhịp's persistent backend as a systemd user service."""
import argparse
import os
from pathlib import Path
import socket
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
UNIT_DIR = Path.home() / '.config' / 'systemd' / 'user'
UNIT = UNIT_DIR / 'nhip-backend.service'
ENV_FILE = ROOT / '.data' / 'server.env'


def systemd_arg(value: str) -> str:
    """Quote one systemd command/environment argument, including '%' specifiers."""
    return '"' + value.replace('\\', '\\\\').replace('"', '\\"').replace('%', '%%') + '"'


def systemd_path(value: str) -> str:
    """Escape path whitespace for non-command systemd directives."""
    return value.replace('\\', '\\\\').replace(' ', '\\x20').replace('\t', '\\x09').replace('%', '%%')


def run(*args: str, check: bool = True) -> subprocess.CompletedProcess[str]:
    return subprocess.run(args, check=check, text=True)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--remove', action='store_true', help='Disable and remove the user service')
    args = parser.parse_args()

    if args.remove:
        run('systemctl', '--user', 'disable', '--now', 'nhip-backend.service', check=False)
        UNIT.unlink(missing_ok=True)
        run('systemctl', '--user', 'daemon-reload')
        print('Removed nhip-backend.service. Database and access key were kept.')
        return 0

    if not (ROOT / 'backend' / 'target' / 'release' / 'nhip-backend').is_file() or not (ROOT / 'dist' / 'index.html').is_file():
        print('Build the release backend and frontend first; see prototype/README.md.', file=sys.stderr)
        return 2
    with socket.socket() as probe:
        if probe.connect_ex(('127.0.0.1', 4876)) == 0:
            print('Port 4876 is occupied. Stop the current backend before installing and starting the service.', file=sys.stderr)
            return 2

    UNIT_DIR.mkdir(mode=0o700, parents=True, exist_ok=True)
    data_dir = ROOT / '.data'
    data_dir.mkdir(mode=0o700, parents=True, exist_ok=True)
    ENV_FILE.touch(mode=0o600, exist_ok=True)
    os.chmod(ENV_FILE, 0o600)
    python = sys.executable
    unit = '\n'.join([
        '[Unit]',
        'Description=Nhịp personal task server',
        'After=network-online.target',
        '',
        '[Service]',
        'Type=simple',
        f'WorkingDirectory={systemd_path(str(ROOT))}',
        f'EnvironmentFile=-{systemd_path(str(ENV_FILE))}',
        f'ExecStart={systemd_arg(python)} {systemd_arg(str(ROOT / "scripts" / "run.py"))} --server-only',
        'Restart=on-failure',
        'RestartSec=5s',
        'UMask=0077',
        'NoNewPrivileges=true',
        '',
        '[Install]',
        'WantedBy=default.target',
        '',
    ])
    UNIT.write_text(unit, encoding='utf-8')
    os.chmod(UNIT, 0o600)
    run('systemctl', '--user', 'daemon-reload')
    run('systemctl', '--user', 'enable', '--now', 'nhip-backend.service')
    print(f'Installed and started {UNIT}.')
    print(f'Optional backend settings: {ENV_FILE} (mode 0600).')
    print('To remove the service without deleting personal data: python3 scripts/install_user_service.py --remove')
    return 0


if __name__ == '__main__':
    try:
        raise SystemExit(main())
    except subprocess.CalledProcessError as exc:
        raise SystemExit(exc.returncode or 1)
