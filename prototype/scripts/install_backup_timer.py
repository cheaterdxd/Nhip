#!/usr/bin/env python3
"""Configure a daily user-level systemd timer for encrypted rclone backups."""
import argparse
import os
from pathlib import Path
import shutil
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
UNIT_DIR = Path.home() / '.config' / 'systemd' / 'user'
SERVICE = UNIT_DIR / 'nhip-drive-backup.service'
TIMER = UNIT_DIR / 'nhip-drive-backup.timer'
ENV_FILE = ROOT / '.data' / 'backup.env'


def systemd_arg(value: str) -> str:
    return '"' + value.replace('\\', '\\\\').replace('"', '\\"').replace('%', '%%') + '"'


def systemd_path(value: str) -> str:
    return value.replace('\\', '\\\\').replace(' ', '\\x20').replace('\t', '\\x09').replace('%', '%%')


def env_value(value: str) -> str:
    return '"' + value.replace('\\', '\\\\').replace('"', '\\"') + '"'


def run(*args: str, check: bool = True) -> subprocess.CompletedProcess[str]:
    return subprocess.run(args, check=check, text=True)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--remote', help='Configured rclone destination, for example gdrive:nhip-backups')
    parser.add_argument('--public-key', type=Path, help='GPG public recovery key; its private key stays offline')
    parser.add_argument('--database', type=Path, default=ROOT / '.data' / 'nhip.sqlite')
    parser.add_argument('--update', action='store_true', help='Replace the existing backup settings')
    parser.add_argument('--remove', action='store_true', help='Disable timer and remove units/settings')
    args = parser.parse_args()

    if args.remove:
        run('systemctl', '--user', 'disable', '--now', 'nhip-drive-backup.timer', check=False)
        run('systemctl', '--user', 'stop', 'nhip-drive-backup.service', check=False)
        SERVICE.unlink(missing_ok=True)
        TIMER.unlink(missing_ok=True)
        ENV_FILE.unlink(missing_ok=True)
        run('systemctl', '--user', 'daemon-reload')
        print('Removed the backup timer and local schedule settings; Drive files and database were kept.')
        return 0

    if not args.remote or ':' not in args.remote or args.remote.startswith(':'):
        parser.error('--remote must be an rclone remote path, such as gdrive:nhip-backups')
    if not args.public_key or not args.public_key.is_file():
        parser.error('--public-key must point to the offline recovery public key')
    if not args.database.is_file():
        parser.error('--database must point to the existing SQLite database')
    if not shutil.which('rclone') or not shutil.which('gpg'):
        parser.error('Install rclone and GnuPG first')
    remote_name = args.remote.partition(':')[0] + ':'
    remotes = subprocess.check_output(['rclone', 'listremotes'], text=True).splitlines()
    if remote_name not in remotes:
        parser.error(f'rclone remote {remote_name!r} is not configured for this Linux user')
    if ENV_FILE.exists() and not args.update:
        parser.error(f'{ENV_FILE} already exists; pass --update to replace its settings')

    UNIT_DIR.mkdir(mode=0o700, parents=True, exist_ok=True)
    ENV_FILE.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
    settings = {
        'NHIP_BACKUP_DATABASE': str(args.database.resolve()),
        'NHIP_BACKUP_REMOTE': args.remote,
        'NHIP_BACKUP_PUBLIC_KEY': str(args.public_key.resolve()),
        'NHIP_BACKUP_KEEP_DAILY': '7',
        'NHIP_BACKUP_KEEP_WEEKLY': '4',
        'NHIP_BACKUP_KEEP_MONTHLY': '3',
    }
    ENV_FILE.write_text(''.join(f'{name}={env_value(value)}\n' for name, value in settings.items()), encoding='utf-8')
    os.chmod(ENV_FILE, 0o600)

    service = '\n'.join([
        '[Unit]', 'Description=Encrypted Nhịp backup to Google Drive', 'After=network-online.target', '',
        '[Service]', 'Type=oneshot', 'UMask=0077', 'NoNewPrivileges=true', 'TimeoutStartSec=2h',
        f'WorkingDirectory={systemd_path(str(ROOT))}',
        f'EnvironmentFile={systemd_path(str(ENV_FILE))}',
        f'ExecStart={systemd_arg(sys.executable)} {systemd_arg(str(ROOT / "scripts" / "backup_drive.py"))}',
        '',
    ])
    timer = '\n'.join([
        '[Unit]', 'Description=Daily encrypted Nhịp backup', '',
        '[Timer]', 'OnCalendar=*-*-* 02:00:00', 'Persistent=true', 'RandomizedDelaySec=5m',
        'Unit=nhip-drive-backup.service', '',
        '[Install]', 'WantedBy=timers.target', '',
    ])
    SERVICE.write_text(service, encoding='utf-8')
    TIMER.write_text(timer, encoding='utf-8')
    os.chmod(SERVICE, 0o600)
    os.chmod(TIMER, 0o600)
    run('systemctl', '--user', 'daemon-reload')
    run('systemctl', '--user', 'enable', '--now', 'nhip-drive-backup.timer')
    print(f'Installed daily backup timer at 02:00 local time: {TIMER}')
    print(f'Backup settings are stored in {ENV_FILE} (mode 0600).')
    print('Review status with: systemctl --user list-timers nhip-drive-backup.timer')
    print('Run once now with: systemctl --user start nhip-drive-backup.service')
    print('Remove with: python3 scripts/install_backup_timer.py --remove')
    return 0


if __name__ == '__main__':
    try:
        raise SystemExit(main())
    except (OSError, subprocess.CalledProcessError) as exc:
        print(f'Could not configure the backup timer: {exc}', file=sys.stderr)
        raise SystemExit(1)
