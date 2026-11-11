#!/usr/bin/env python3
"""Create, verify, or restore an encrypted SQLite snapshot without touching the live DB."""
import argparse
import getpass
import hashlib
import json
import os
from pathlib import Path
import shutil
import sqlite3
import subprocess
import sys
import tarfile
import tempfile
from datetime import datetime, timezone

APP_VERSION = "0.1.0"
FORMAT = "nhip-encrypted-backup-v1"
MAX_ARCHIVE_BYTES = 4 * 1024 * 1024 * 1024


def digest(path: Path) -> str:
    value = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            value.update(block)
    return value.hexdigest()


def run_gpg(args: list[str], *, passphrase: str | None, gpg_home: Path) -> None:
    command = ["gpg", "--no-options", "--homedir", str(gpg_home), "--batch", "--yes", "--no-symkey-cache"]
    if passphrase is not None:
        command.extend(["--pinentry-mode", "loopback", "--passphrase-fd", "0"])
    result = subprocess.run(
        [*command, *args], input=(passphrase + "\n") if passphrase is not None else None,
        stdout=subprocess.DEVNULL, stderr=subprocess.PIPE,
        check=False, text=True, timeout=300,
    )
    if result.returncode:
        # GnuPG diagnostics can include paths and recipient information, but never echo secrets.
        raise RuntimeError("GnuPG failed: " + (result.stderr.strip()[-600:] or "unknown error"))


def make_snapshot(database: Path, destination: Path) -> dict:
    source = sqlite3.connect(database.resolve().as_uri() + "?mode=ro", uri=True, timeout=30)
    target = sqlite3.connect(destination)
    try:
        source.backup(target, pages=256, sleep=0.05)
        target.execute("PRAGMA foreign_keys=ON")
        target.execute("BEGIN IMMEDIATE")
        tables = {row[0] for row in target.execute("SELECT name FROM sqlite_master WHERE type='table'")}
        # Device endpoints and delivery tokens are credentials; restore requires devices to register again.
        for table in ("app_push_delivery", "app_weekly_report_delivery", "push_delivery", "devices"):
            if table in tables:
                target.execute(f'DELETE FROM "{table}"')
        target.commit()
        target.execute("PRAGMA wal_checkpoint(TRUNCATE)")
        target.execute("PRAGMA journal_mode=DELETE")
        check = target.execute("PRAGMA quick_check").fetchone()[0]
        if check != "ok":
            raise RuntimeError("SQLite snapshot integrity check failed")
        version = target.execute("PRAGMA user_version").fetchone()[0]
        counts = {}
        for table in sorted(tables):
            # Table names come from sqlite_master, then are quoted as identifiers.
            counts[table] = target.execute(f'SELECT count(*) FROM "{table}"').fetchone()[0]
    finally:
        target.close()
        source.close()
    return {
        "format": FORMAT,
        "created_at": datetime.now(timezone.utc).isoformat(),
        "app_version": APP_VERSION,
        "sqlite_user_version": version,
        "database_bytes": destination.stat().st_size,
        "database_sha256": digest(destination),
        "table_counts": counts,
        "excluded": ["registered devices", "web push subscriptions", "push delivery credentials"],
    }


def validate_archive(archive: Path, gpg_home: Path, passphrase: str, work: Path) -> tuple[Path, dict]:
    tar_path = work / "decrypted.tar.gz"
    run_gpg(["--decrypt", "--output", str(tar_path), str(archive)], passphrase=passphrase, gpg_home=gpg_home)
    if tar_path.stat().st_size > MAX_ARCHIVE_BYTES:
        raise RuntimeError("Decrypted archive exceeds 4 GiB limit")
    with tarfile.open(tar_path, "r:gz") as tar:
        members = tar.getmembers()
        names = {member.name for member in members}
        if len(members) != 2 or names != {"manifest.json", "database.sqlite"} or any(not member.isfile() for member in members):
            raise RuntimeError("Archive contains unexpected files")
        manifest_member = tar.getmember("manifest.json")
        database_member = tar.getmember("database.sqlite")
        if manifest_member.size > 1024 * 1024 or database_member.size > MAX_ARCHIVE_BYTES:
            raise RuntimeError("Archive member exceeds allowed size")
        manifest_stream = tar.extractfile(manifest_member)
        database_stream = tar.extractfile(database_member)
        if manifest_stream is None or database_stream is None:
            raise RuntimeError("Archive is incomplete")
        manifest = json.load(manifest_stream)
        if not isinstance(manifest, dict):
            raise RuntimeError("Backup manifest is invalid")
        database_path = work / "database.sqlite"
        with database_path.open("xb") as out:
            shutil.copyfileobj(database_stream, out, 1024 * 1024)
    if manifest.get("format") != FORMAT or manifest.get("database_bytes") != database_path.stat().st_size:
        raise RuntimeError("Backup manifest does not match database")
    if manifest.get("database_sha256") != digest(database_path):
        raise RuntimeError("Backup checksum does not match")
    with sqlite3.connect(database_path.resolve().as_uri() + "?mode=ro", uri=True) as db:
        if db.execute("PRAGMA quick_check").fetchone()[0] != "ok":
            raise RuntimeError("Restored database failed SQLite integrity check")
        if db.execute("PRAGMA foreign_key_check").fetchone() is not None:
            raise RuntimeError("Restored database has foreign key violations")
        tables = {row[0] for row in db.execute("SELECT name FROM sqlite_master WHERE type='table'")}
        if not {"app_projects", "app_items", "app_task_reminders"}.issubset(tables):
            raise RuntimeError("Database is missing core Nhịp tables")
    return database_path, manifest


def import_key(path: Path, gpg_home: Path) -> str:
    run_gpg(["--import", str(path)], passphrase=None, gpg_home=gpg_home)
    listing = subprocess.run(
        ["gpg", "--no-options", "--homedir", str(gpg_home), "--batch", "--with-colons", "--fingerprint", "--list-keys"],
        capture_output=True, text=True, check=False, timeout=60,
    )
    if listing.returncode:
        raise RuntimeError("Could not inspect imported backup key")
    records = listing.stdout.splitlines()
    primary_keys = [index for index, line in enumerate(records) if line.startswith("pub:")]
    fingerprints = []
    if len(primary_keys) == 1:
        for line in records[primary_keys[0] + 1:]:
            if line.startswith(("pub:", "sub:")):
                break
            if line.startswith("fpr:"):
                fields = line.split(":")
                if len(fields) > 9:
                    fingerprints.append(fields[9])
                    break
    if len(fingerprints) != 1 or not fingerprints[0]:
        raise RuntimeError("Provide a key file containing exactly one primary backup key")
    return fingerprints[0]


def verify_ciphertext(path: Path) -> None:
    if not path.is_file() or path.stat().st_size < 128:
        raise RuntimeError("Encrypted backup output is empty or incomplete")


def get_passphrase(confirm: bool = False) -> str:
    value = getpass.getpass("Backup recovery passphrase (not stored on this PC): ")
    if len(value) < 16:
        raise RuntimeError("Use a recovery passphrase of at least 16 characters")
    if confirm and getpass.getpass("Repeat passphrase: ") != value:
        raise RuntimeError("Passphrases do not match")
    return value


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    export = commands.add_parser("export", help="Create and verify an encrypted consistent snapshot")
    export.add_argument("--database", type=Path, required=True)
    export.add_argument("--output", type=Path, required=True, help="New .gpg destination; existing files are never overwritten")
    export.add_argument("--recipient-public-key", type=Path, help="Encrypt for an offline GPG recovery key; enables unattended export")
    verify = commands.add_parser("verify", help="Decrypt a backup temporarily and verify its checksum/schema")
    verify.add_argument("--input", type=Path, required=True)
    verify.add_argument("--identity", type=Path, help="Private recovery key file for recipient-encrypted backups")
    restore = commands.add_parser("restore", help="Restore to a new database path; never overwrites an existing file")
    restore.add_argument("--input", type=Path, required=True)
    restore.add_argument("--target", type=Path, required=True)
    restore.add_argument("--identity", type=Path, help="Private recovery key file for recipient-encrypted backups")
    args = parser.parse_args()
    os.umask(0o077)
    try:
        if shutil.which("gpg") is None:
            raise RuntimeError("Install GnuPG (`gpg`) on the server first")
        if args.command == "export" and (not args.database.is_file() or args.output.exists()):
            raise RuntimeError("Database must exist and output must be a new file")
        if args.command == "export" and args.recipient_public_key and not args.recipient_public_key.is_file():
            raise RuntimeError("Public recovery key file does not exist")
        if args.command == "restore" and (not args.input.is_file() or args.target.exists()):
            raise RuntimeError("Backup must exist and restore target must be a new file")
        if args.command == "verify" and not args.input.is_file():
            raise RuntimeError("Backup file does not exist")
        if args.command in {"verify", "restore"} and args.identity and not args.identity.is_file():
            raise RuntimeError("Private recovery key file does not exist")
        passphrase = None if args.command == "export" and args.recipient_public_key else get_passphrase(confirm=args.command == "export")
        if args.command == "export":
            args.output.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
        elif args.command == "restore":
            args.target.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(prefix="nhip-backup-") as raw:
            work = Path(raw)
            work.chmod(0o700)
            gpg_home = work / "gnupg"
            gpg_home.mkdir(mode=0o700)
            if args.command == "export":
                recipient = import_key(args.recipient_public_key, gpg_home) if args.recipient_public_key else None
                snapshot = work / "database.sqlite"
                manifest = make_snapshot(args.database, snapshot)
                manifest_path = work / "manifest.json"
                manifest_path.write_text(json.dumps(manifest, ensure_ascii=False, sort_keys=True), encoding="utf-8")
                manifest_path.chmod(0o600)
                tar_path = work / "archive.tar.gz"
                with tarfile.open(tar_path, "w:gz") as tar:
                    tar.add(manifest_path, arcname="manifest.json")
                    tar.add(snapshot, arcname="database.sqlite")
                temp_encrypted = work / "backup.gpg"
                encryption = ["--compress-algo", "none", "--output", str(temp_encrypted)]
                if recipient:
                    encryption.extend(["--trust-model", "always", "--encrypt", "--recipient", recipient])
                else:
                    encryption.extend(["--symmetric", "--cipher-algo", "AES256"])
                encryption.append(str(tar_path))
                run_gpg(encryption, passphrase=passphrase, gpg_home=gpg_home)
                if recipient:
                    verify_ciphertext(temp_encrypted)
                else:
                    check_dir = work / "verification"
                    check_dir.mkdir(mode=0o700)
                    checked, checked_manifest = validate_archive(temp_encrypted, gpg_home, passphrase, check_dir)
                    if checked_manifest != manifest:
                        raise RuntimeError("Post-encryption verification failed")
                    with sqlite3.connect(checked.resolve().as_uri() + "?mode=ro", uri=True) as db:
                        if db.execute("PRAGMA quick_check").fetchone()[0] != "ok":
                            raise RuntimeError("Encrypted backup verification failed")
                # Publish only ciphertext to the destination, and refuse to replace an existing file.
                fd = os.open(args.output, os.O_CREAT | os.O_EXCL | os.O_WRONLY, 0o600)
                try:
                    with os.fdopen(fd, "wb") as out, temp_encrypted.open("rb") as encrypted:
                        shutil.copyfileobj(encrypted, out, 1024 * 1024)
                        out.flush()
                        os.fsync(out.fileno())
                except BaseException:
                    args.output.unlink(missing_ok=True)
                    raise
                print(f"Encrypted backup created: {args.output}")
                print(f"Database SHA-256: {manifest['database_sha256']}")
                if recipient:
                    print("Snapshot checks passed before public-key encryption; ciphertext decryption requires the private identity.")
                else:
                    print("Encrypted backup passed an immediate decrypt and integrity check.")
                print("Device registrations and push credentials were removed; register devices again after restore.")
            else:
                if args.identity:
                    import_key(args.identity, gpg_home)
                    passphrase = get_passphrase()
                check_dir = work / "verification"
                check_dir.mkdir(mode=0o700)
                checked, manifest = validate_archive(args.input, gpg_home, passphrase, check_dir)
                if args.command == "restore":
                    args.target.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
                    fd = os.open(args.target, os.O_CREAT | os.O_EXCL | os.O_WRONLY, 0o600)
                    try:
                        with os.fdopen(fd, "wb") as out, checked.open("rb") as restored:
                            shutil.copyfileobj(restored, out, 1024 * 1024)
                            out.flush()
                            os.fsync(out.fileno())
                    except BaseException:
                        args.target.unlink(missing_ok=True)
                        raise
                    print(f"Verified database restored to a new path: {args.target}")
                    print("Review it, then stop the backend before choosing how to make it active.")
                    print("Device registrations and push credentials must be recreated.")
                else:
                    print(f"Backup verified: {args.input}")
                    print(f"Created at: {manifest['created_at']} · app {manifest['app_version']} · SQLite schema {manifest['sqlite_user_version']}")
        return 0
    except (OSError, sqlite3.Error, tarfile.TarError, json.JSONDecodeError, subprocess.TimeoutExpired, RuntimeError) as exc:
        print(f"Backup operation failed: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
