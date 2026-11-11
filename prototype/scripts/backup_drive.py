#!/usr/bin/env python3
"""Create an encrypted SQLite backup, upload and verify it with rclone, then prune old copies."""
from datetime import datetime, timezone
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile
import uuid

ROOT = Path(__file__).resolve().parents[1]
DATA = ROOT / '.data'
DB = Path(os.environ.get('NHIP_BACKUP_DATABASE', str(DATA / 'nhip.sqlite'))).expanduser()
REMOTE = os.environ.get('NHIP_BACKUP_REMOTE', '')
PUBLIC_KEY = Path(os.environ.get('NHIP_BACKUP_PUBLIC_KEY', '')).expanduser()
DAILY = int(os.environ.get('NHIP_BACKUP_KEEP_DAILY', '7'))
WEEKLY = int(os.environ.get('NHIP_BACKUP_KEEP_WEEKLY', '4'))
MONTHLY = int(os.environ.get('NHIP_BACKUP_KEEP_MONTHLY', '3'))
STATUS = DATA / 'backup-status.json'
NAME = re.compile(r'^nhip-(\d{8}T\d{6}Z)-([0-9a-f]{8})\.gpg$')


def command(args: list[str], timeout: int = 3600) -> str:
    result = subprocess.run(args, text=True, capture_output=True, timeout=timeout, check=False)
    if result.returncode:
        details = (result.stderr.strip() or result.stdout.strip())[-800:]
        raise RuntimeError(f"Command failed ({Path(args[0]).name}): {details}")
    return result.stdout


def remote_path(name: str = '') -> str:
    return REMOTE.rstrip('/') + (('/' + name) if name else '')


def md5(path: Path) -> str:
    value = hashlib.md5()
    with path.open('rb') as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b''):
            value.update(block)
    return value.hexdigest()


def parse_name(name: str) -> datetime | None:
    match = NAME.fullmatch(name)
    if not match:
        return None
    return datetime.strptime(match.group(1), '%Y%m%dT%H%M%SZ').replace(tzinfo=timezone.utc)


def keep_set(names: list[str]) -> set[str]:
    dated = [(parse_name(name), name) for name in names]
    dated = sorted(((date, name) for date, name in dated if date is not None), reverse=True)
    keep: set[str] = set()
    for key_fn, limit in (
        (lambda d: d.date(), DAILY),
        (lambda d: (d.isocalendar().year, d.isocalendar().week), WEEKLY),
        (lambda d: (d.year, d.month), MONTHLY),
    ):
        seen = set()
        buckets = 0
        for date, name in dated:
            bucket = key_fn(date.astimezone())
            if bucket in seen:
                continue
            seen.add(bucket)
            if buckets < limit:
                keep.add(name)
                buckets += 1
    return keep


def write_status(state: str, *, detail: str = '', backup: str | None = None) -> None:
    DATA.mkdir(mode=0o700, parents=True, exist_ok=True)
    try:
        previous = json.loads(STATUS.read_text(encoding='utf-8')) if STATUS.exists() else {}
        if not isinstance(previous, dict):
            previous = {}
    except (OSError, json.JSONDecodeError):
        previous = {}
    now = datetime.now(timezone.utc).isoformat()
    record = {**previous, 'last_attempt': now, 'status': state, 'detail': detail[:500]}
    if state == 'success':
        record.update(last_success=now, latest_backup=backup)
    temp = STATUS.with_suffix('.tmp')
    temp.write_text(json.dumps(record, ensure_ascii=False, sort_keys=True), encoding='utf-8')
    os.chmod(temp, 0o600)
    temp.replace(STATUS)


def main() -> int:
    argparse.ArgumentParser(description=__doc__).parse_args()
    try:
        if not DB.is_file():
            raise RuntimeError(f'Backup database does not exist: {DB}')
        if not PUBLIC_KEY.is_file():
            raise RuntimeError('Set NHIP_BACKUP_PUBLIC_KEY to the offline recovery public-key file.')
        if not REMOTE or ':' not in REMOTE or REMOTE.startswith(':'):
            raise RuntimeError('Set NHIP_BACKUP_REMOTE to an rclone Drive remote and folder, for example gdrive:nhip-backups.')
        if min(DAILY, WEEKLY, MONTHLY) < 1:
            raise RuntimeError('Retention counts must all be at least one.')
        if not shutil.which('rclone'):
            raise RuntimeError('Install rclone and configure its Google Drive remote first.')
        if not shutil.which('gpg'):
            raise RuntimeError('Install GnuPG (gpg) first.')
    except Exception as exc:
        try:
            write_status('failed', detail=str(exc))
        except OSError:
            pass
        raise

    DATA.mkdir(mode=0o700, parents=True, exist_ok=True)
    os.chmod(DATA, 0o700)
    stamp = datetime.now(timezone.utc).strftime('%Y%m%dT%H%M%SZ')
    filename = f'nhip-{stamp}-{uuid.uuid4().hex[:8]}.gpg'
    try:
        with tempfile.TemporaryDirectory(prefix='nhip-drive-backup-', dir=DATA) as folder:
            encrypted = Path(folder) / filename
            command([sys.executable, str(ROOT / 'scripts' / 'backup.py'), 'export',
                     '--database', str(DB), '--output', str(encrypted),
                     '--recipient-public-key', str(PUBLIC_KEY)], timeout=600)
            command(['rclone', 'mkdir', remote_path()], timeout=300)
            remote_file = remote_path(filename)
            command(['rclone', 'copyto', str(encrypted), remote_file, '--immutable'], timeout=1800)
            remote_hash = command(['rclone', 'hashsum', 'MD5', remote_file], timeout=300).strip().split()
            if not remote_hash or remote_hash[0].lower() != md5(encrypted):
                raise RuntimeError('The uploaded backup MD5 does not match the local ciphertext.')
            listed = command(['rclone', 'lsf', '--files-only', remote_path()], timeout=300)
            names = [line.strip() for line in listed.splitlines() if NAME.fullmatch(line.strip())]
            if filename not in names:
                raise RuntimeError('The uploaded backup did not appear in the remote listing.')
            keep = keep_set(names)
            delete = sorted(set(names) - keep)
            for old in delete:
                command(['rclone', 'deletefile', remote_path(old)], timeout=300)
        write_status('success', detail=f'Uploaded and verified; retained {len(keep)} copies.', backup=filename)
        print(f'Encrypted backup uploaded and verified: {remote_path(filename)}')
        print(f'Retention applied after verification; retained {len(keep)} timestamped backups.')
        return 0
    except Exception as exc:
        try:
            write_status('failed', detail=str(exc))
        except OSError:
            pass
        raise


if __name__ == '__main__':
    try:
        raise SystemExit(main())
    except (OSError, RuntimeError, subprocess.TimeoutExpired, subprocess.SubprocessError, ValueError) as exc:
        print(f'Google Drive backup failed: {exc}', file=sys.stderr)
        raise SystemExit(1)
