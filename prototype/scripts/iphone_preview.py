#!/usr/bin/env python3
"""Start a disposable, authenticated prototype behind tailnet-only HTTPS.

This owns its backend process and foreground Serve; Ctrl-C stops both. It refuses
an existing Serve configuration and never exposes real user data or secrets.
"""
import argparse
import base64
import json
import os
from pathlib import Path
import secrets
import socket
import subprocess
import sys
import time
import urllib.request

ROOT = Path(__file__).resolve().parents[1]
PORT = 4879


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--show-key', action='store_true', help='Show local preview key to enter on iPhone')
    args = parser.parse_args()
    private = ROOT / '.data' / 'iphone-preview'
    private.mkdir(mode=0o700, parents=True, exist_ok=True)
    private.chmod(0o700)
    keyfile, pemfile = private / 'access-key', private / 'vapid.pem'
    if args.show_key:
        if not keyfile.exists():
            sys.exit('Preview has not been initialized yet.')
        print(keyfile.read_text().strip())
        return
    cfg = json.loads(subprocess.check_output(['tailscale', 'serve', 'status', '--json'], text=True) or '{}')
    if cfg:
        sys.exit('Tailscale Serve already has configuration; preserve it and select a separate setup manually.')
    status = json.loads(subprocess.check_output(['tailscale', 'status', '--json'], text=True))
    if status.get('BackendState') != 'Running' or not status.get('CertDomains'):
        sys.exit('Tailscale must be running with HTTPS enabled on the tailnet.')
    with socket.socket() as sock:
        if sock.connect_ex(('127.0.0.1', PORT)) == 0:
            sys.exit(f'Local port {PORT} is already occupied.')
    if not keyfile.exists():
        fd = os.open(keyfile, os.O_CREAT | os.O_EXCL | os.O_WRONLY, 0o600)
        with os.fdopen(fd, 'w') as f:
            f.write(secrets.token_urlsafe(32))
    if not pemfile.exists():
        fd = os.open(pemfile, os.O_CREAT | os.O_EXCL | os.O_WRONLY, 0o600)
        with os.fdopen(fd, 'wb') as f:
            subprocess.run(['openssl', 'ecparam', '-name', 'prime256v1', '-genkey', '-noout'], stdout=f, check=True)
    der = subprocess.check_output(['openssl', 'ec', '-in', str(pemfile), '-pubout', '-outform', 'DER'], stderr=subprocess.DEVNULL)
    public = der[-65:]
    if len(public) != 65 or public[0] != 4:
        sys.exit('Invalid VAPID public key.')
    env = {**os.environ, 'NHIP_DB': str(private / 'fixture.sqlite'), 'NHIP_DIST': str(ROOT / 'dist'),
           'NHIP_PORT': str(PORT), 'NHIP_ACCESS_KEY': keyfile.read_text().strip(),
           'NHIP_VAPID_PEM': str(pemfile), 'NHIP_VAPID_PUBLIC': base64.urlsafe_b64encode(public).decode().rstrip('='),
           'NHIP_VAPID_SUBJECT': 'mailto:admin@example.com'}
    binary = ROOT / 'backend' / 'target' / 'release' / ('nhip-backend.exe' if os.name == 'nt' else 'nhip-backend')
    if not binary.is_file() or not (ROOT / 'dist/index.html').is_file():
        sys.exit('Build frontend and backend first; see README.')
    backend = subprocess.Popen([str(binary)], cwd=ROOT, env=env)
    serve = None
    try:
        for _ in range(200):
            if backend.poll() is not None:
                raise RuntimeError('Backend quit before becoming ready.')
            request = urllib.request.Request(f'http://127.0.0.1:{PORT}/api/health', headers={'Authorization': 'Bearer '+env['NHIP_ACCESS_KEY']})
            try:
                with urllib.request.urlopen(request, timeout=1) as response:
                    if json.load(response).get('prototype'):
                        break
            except OSError:
                time.sleep(.05)
        else:
            raise TimeoutError('Backend did not start.')
        domain = status['CertDomains'][0]
        print(f'Open https://{domain}/ from your tailnet.', flush=True)
        print('In a separate terminal run: python3 scripts/iphone_preview.py --show-key', flush=True)
        print('Ctrl-C stops this preview. The key and VAPID material stay in .data/iphone-preview.', flush=True)
        serve = subprocess.Popen(['tailscale', 'serve', '--https=443', f'http://127.0.0.1:{PORT}'])
        while serve.poll() is None and backend.poll() is None:
            time.sleep(.5)
        if serve.poll() is not None:
            raise RuntimeError(f'Tailscale Serve exited ({serve.returncode}); HTTPS may need enabling in admin settings.')
        raise RuntimeError('Backend exited unexpectedly.')
    except KeyboardInterrupt:
        pass
    finally:
        for proc in (serve, backend):
            if proc and proc.poll() is None:
                proc.terminate()
                try:
                    proc.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    proc.kill()
                    proc.wait()


if __name__ == '__main__':
    main()
