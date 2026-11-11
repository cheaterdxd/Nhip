#!/usr/bin/env python3
"""Exercise reminder API against a disposable database; never touches user data."""
import json
import os
from pathlib import Path
import socket
import sqlite3
import subprocess
import tempfile
import threading
import time
import urllib.error
import urllib.request

ROOT = Path(__file__).resolve().parents[1]
TOKEN = 'temporary-smoke-test-key'


def main():
    with socket.socket() as sock:
        sock.bind(('127.0.0.1', 0))
        port = sock.getsockname()[1]
    with tempfile.TemporaryDirectory(prefix='nhip-reminders-') as directory:
        env = {**os.environ, 'NHIP_DB': str(Path(directory) / 'fixture.sqlite'),
               'NHIP_DIST': str(ROOT / 'dist'), 'NHIP_PORT': str(port), 'NHIP_ACCESS_KEY': TOKEN}
        base = f'http://127.0.0.1:{port}'
        process = None

        def call(path, data=None, authenticated=True):
            headers = {'authorization': 'Bearer ' + TOKEN} if authenticated else {}
            if data is not None:
                headers['content-type'] = 'application/json'
                data = json.dumps(data).encode()
            request = urllib.request.Request(base + path, data=data, headers=headers)
            try:
                with urllib.request.urlopen(request, timeout=30) as response:
                    return response.status, json.load(response)
            except urllib.error.HTTPError as error:
                return error.code, error.read().decode()

        def start():
            proc = subprocess.Popen([str(ROOT / 'backend/target/release/nhip-backend')],
                                    cwd=ROOT, env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            for _ in range(200):
                if proc.poll() is not None:
                    raise RuntimeError('Backend exited before becoming ready')
                try:
                    if call('/api/health')[0] == 200:
                        return proc
                except OSError:
                    time.sleep(.05)
            proc.terminate()
            proc.wait(timeout=5)
            raise TimeoutError('Backend startup timed out')

        def stop(proc):
            if proc and proc.poll() is None:
                proc.terminate()
                proc.wait(timeout=5)

        try:
            process = start()
            assert call('/api/health', authenticated=False)[0] == 401
            for device in ('desktop-a', 'phone-b'):
                assert call('/api/devices', {'id': device, 'channel': 'browser'})[0] == 200
            payload = {'request_id': 'stable-id', 'task_id': 9501, 'delay_seconds': 0}
            created = call('/api/reminders', payload)
            assert created[0] == 200 and call('/api/reminders', payload) == created
            reminder_id = created[1]['id']
            conflict = {**payload, 'task_id': 9502}
            assert call('/api/reminders', conflict)[0] == 409
            # An old unanswered reminder must stay visible even after >200 newer
            # reminders have already been acknowledged.
            with sqlite3.connect(Path(directory) / 'fixture.sqlite') as fixture:
                fixture.executemany(
                    'INSERT INTO reminders(request_id,task_id,due,ack_device,announced) VALUES(?,9501,0,?,1)',
                    [(f'newer-ack-{index}', 'phone-b') for index in range(205)])
            status, initial = call('/api/devices/desktop-a/updates')
            assert status == 200 and any(item['id'] == reminder_id and item['ack_device'] is None
                                         for item in initial['reminders'])
            received = []
            watcher = threading.Thread(target=lambda: received.append(call(
                f"/api/devices/desktop-a/updates?since={initial['revision']}")), daemon=True)
            watcher.start()
            time.sleep(.2)
            assert call(f'/api/reminders/{reminder_id}/ack', {'device_id': 'phone-b'})[1]['ack_device'] == 'phone-b'
            watcher.join(timeout=4)
            assert not watcher.is_alive() and received and any(
                item['id'] == reminder_id and item['ack_device'] == 'phone-b'
                for item in received[0][1]['reminders'])
            assert call(f'/api/reminders/{reminder_id}/ack', {'device_id': 'desktop-a'})[1]['ack_device'] == 'phone-b'
            stop(process)
            process = start()
            assert call('/api/reminders', payload)[1]['id'] == reminder_id
            _, after_restart = call('/api/devices/desktop-a/updates')
            assert any(item['id'] == reminder_id and item['ack_device'] == 'phone-b'
                       for item in after_restart['reminders'])
            print('PASS: auth, registration, ID deduplication, cross-device wake/ack, restart and reconnect')
        finally:
            stop(process)


if __name__ == '__main__':
    main()
