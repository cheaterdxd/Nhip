#!/usr/bin/env python3
"""Start the matching local backend before opening the packaged Tauri client."""
import os
from pathlib import Path
import socket
import subprocess
import sys
from benchmark import Client, wait_ready

ROOT=Path(__file__).resolve().parents[1]
backend_binary=ROOT/'backend/target/release/nhip-backend'
desktop_binary=ROOT/'src-tauri/target/release/nhip-desktop'
if not backend_binary.exists() or not desktop_binary.exists() or not (ROOT/'dist/index.html').exists():
    sys.exit('Build the release binaries and frontend first; see prototype/README.md.')
backend=None
desktop=None
try:
    with socket.socket() as probe:
        occupied=probe.connect_ex(('127.0.0.1',4876))==0
    if occupied:
        c=Client(4876)
        try:
            _,health=c.call('GET','/api/health')
            if not health.get('prototype'):
                raise RuntimeError('Port 4876 belongs to a different service.')
        finally:c.close()
        print('Using the running Nhịp prototype backend.',flush=True)
    else:
        env={**os.environ,'NHIP_DB':str(ROOT/'.data/prototype.sqlite'),'NHIP_DIST':str(ROOT/'dist')}
        backend=subprocess.Popen([str(backend_binary)],cwd=ROOT,env=env)
        wait_ready(4876,backend)
    env={k:v for k,v in os.environ.items() if not k.startswith('NHIP_BENCH_')}
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
