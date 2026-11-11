#!/usr/bin/env python3
"""Start Nhịp's backend, optionally opening the packaged Tauri client."""
import os
import argparse
import re
import secrets
import shlex
from pathlib import Path
import socket
import subprocess
import sys
from benchmark import Client, wait_ready

ROOT=Path(__file__).resolve().parents[1]
suffix='.exe' if os.name=='nt' else ''
DATA=ROOT/'.data'
KEY_FILE=DATA/'access-key'
DB_FILE=DATA/'nhip.sqlite'
backend_binary=ROOT/'backend/target/release'/('nhip-backend'+suffix)
desktop_binary=ROOT/'src-tauri/target/release'/('nhip-desktop'+suffix)
backend=None
desktop=None

def runtime_environment():
    env=os.environ.copy()
    path=DATA/'server.env'
    if path.is_file():
        for number,line in enumerate(path.read_text(encoding='utf-8').splitlines(),1):
            line=line.strip()
            if not line or line.startswith('#'):continue
            name,separator,value=line.partition('=')
            if not separator or not re.fullmatch(r'[A-Z_][A-Z0-9_]*',name):
                raise ValueError(f'Invalid setting at {path}:{number}')
            values=shlex.split(value,comments=False,posix=True)
            if len(values)!=1:
                raise ValueError(f'Expected one value at {path}:{number}')
            env.setdefault(name,values[0])
    return env

try:
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--show-key',action='store_true',help='Show the local server access key in this terminal')
    parser.add_argument('--server-only',action='store_true',help='Run the backend in the foreground without opening Tauri')
    args=parser.parse_args()
    if args.show_key:
        key=os.environ.get('NHIP_ACCESS_KEY')
        if not key and KEY_FILE.is_file():key=KEY_FILE.read_text().strip()
        if not key:sys.exit('No access key yet. Start the app once to create one.')
        print(key)
        raise SystemExit(0)
    if not backend_binary.exists() or (not args.server_only and not desktop_binary.exists()) or not (ROOT/'dist/index.html').exists():
        sys.exit('Build the release binaries and frontend first; see prototype/README.md.')
    runtime_env=runtime_environment()
    bind_host=runtime_env.get('NHIP_HOST','127.0.0.1')
    port=int(runtime_env.get('NHIP_PORT','4876'))
    probe_host=bind_host.strip('[]')
    if probe_host in ('0.0.0.0','::'):
        probe_host='127.0.0.1' if probe_host=='0.0.0.0' else '::1'
    with socket.socket(socket.AF_INET6 if ':' in probe_host else socket.AF_INET) as probe:
        occupied=probe.connect_ex((probe_host,port))==0
    key=runtime_env.get('NHIP_ACCESS_KEY')
    if not key and KEY_FILE.is_file():key=KEY_FILE.read_text().strip()
    if occupied:
        if args.server_only:
            sys.exit('Port 4876 is already in use. Stop the existing Nhịp backend before starting the persistent server.')
        c=Client(port,key,probe_host)
        try:
            _,health=c.call('GET','/api/health')
            if not health.get('prototype'):
                raise RuntimeError('Port 4876 belongs to a different service.')
        finally:c.close()
        print('Using the running Nhịp prototype backend.',flush=True)
    else:
        DATA.mkdir(mode=0o700,parents=True,exist_ok=True)
        try:DATA.chmod(0o700)
        except OSError:pass
        key=runtime_env.get('NHIP_ACCESS_KEY')
        if not key:
            if KEY_FILE.exists():key=KEY_FILE.read_text().strip()
            else:
                key=secrets.token_urlsafe(32)
                fd=os.open(KEY_FILE,os.O_CREAT|os.O_EXCL|os.O_WRONLY,0o600)
                with os.fdopen(fd,'w') as f:f.write(key+'\n')
        if KEY_FILE.exists():
            try:KEY_FILE.chmod(0o600)
            except OSError:pass
        if not key:sys.exit('The access key is empty.')
        env={**runtime_env,'NHIP_DB':str(DB_FILE),'NHIP_DIST':str(ROOT/'dist'),'NHIP_ACCESS_KEY':key,'NHIP_SEED_FIXTURE':'0'}
        backend=subprocess.Popen([str(backend_binary)],cwd=ROOT,env=env)
        wait_ready(port,backend,access_key=key,host=probe_host)
        print('Started personal backend using the local database and access key.',flush=True)
        print('Open “Kết nối & thử nhắc việc” and enter the access key on this device.',flush=True)
        print('To show the key for another client: python3 scripts/run.py --show-key',flush=True)
    if args.server_only:
        print(f'Nhịp backend is running on {bind_host}:{port}. Press Ctrl-C to stop.',flush=True)
        if backend is None:
            sys.exit('Server-only mode must own the backend process.')
        code=backend.wait()
        if code:
            raise RuntimeError(f'Backend exited with status {code}.')
        raise SystemExit(0)
    env={k:v for k,v in runtime_env.items() if not k.startswith('NHIP_BENCH_')}
    desktop=subprocess.Popen([str(desktop_binary)],cwd=ROOT,env=env)
    desktop.wait()
except KeyboardInterrupt:
    pass
finally:
    for proc in (desktop,backend):
        if proc and proc.poll() is None:
            proc.terminate()
            try:proc.wait(timeout=5)
            except subprocess.TimeoutExpired:proc.kill();proc.wait()
