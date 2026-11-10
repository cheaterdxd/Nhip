#!/usr/bin/env python3
"""Compare idle process/thread CPU without changing application code.
Uses an already-running prototype backend; starts/stops only its own desktop.
Renderer environment overrides are diagnostic experiments, not product defaults.
"""
import argparse
import json
import os
from pathlib import Path
import subprocess
import time
from benchmark import ROOT, Client, desktop_run, idle_measure, process_table


def threads(root):
    table = process_table()
    pids = {root}
    while True:
        children = {pid for pid, (ppid, _) in table.items() if ppid in pids}
        if children.issubset(pids):
            break
        pids |= children
    result = {}
    for pid in pids:
        for task in (Path('/proc') / str(pid) / 'task').glob('*'):
            try:
                raw = (task / 'stat').read_text()
                fields = raw[raw.rfind(')') + 2:].split()
                result[task.name] = {'pid': pid, 'name': (task / 'comm').read_text().strip(),
                                     'ticks': int(fields[11]) + int(fields[12])}
            except (OSError, ValueError, IndexError):
                continue
    return result


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--seconds', type=int, default=30)
    parser.add_argument('--variant', choices=['default', 'no-dmabuf', 'x11'], default='default')
    args = parser.parse_args()
    c = Client(4876)
    try:
        _, health = c.call('GET', '/api/health')
        if not health.get('prototype'):
            raise RuntimeError('Expected running prototype backend on 4876')
    finally:
        c.close()
    overrides = {'default': {}, 'no-dmabuf': {'WEBKIT_DISABLE_DMABUF_RENDERER': '1'},
                 'x11': {'GDK_BACKEND': 'x11'}}[args.variant]
    os.environ.update(overrides)
    work = ROOT / '.bench-work' / f'idle-{time.strftime("%Y%m%d-%H%M%S")}-{args.variant}'
    work.mkdir(parents=True)
    proc = None
    result = {'variant': args.variant, 'overrides': overrides, 'work_dir': str(work)}
    try:
        proc, result['ready'] = desktop_run(ROOT / 'src-tauri/target/release/nhip-desktop', work, 'diagnostic', stay=True)
        before = threads(proc.pid)
        start = time.monotonic()
        result['idle'] = idle_measure(proc.pid, None, args.seconds)['backend']
        duration = time.monotonic() - start
        after = threads(proc.pid)
        result['thread_cpu'] = sorted([
            {'tid': tid, 'pid': value['pid'], 'name': value['name'],
             'cpu_percent_one_core': (value['ticks'] - before[tid]['ticks']) / os.sysconf('SC_CLK_TCK') / duration * 100}
            for tid, value in after.items() if tid in before
        ], key=lambda v: -v['cpu_percent_one_core'])
        result['completed'] = True
    finally:
        if proc and proc.poll() is None:
            proc.terminate()
            try:
                proc.wait(timeout=5)
            except subprocess.TimeoutExpired:
                proc.kill()
                proc.wait()
        output = ROOT / 'results' / f'idle-{work.name}.json'
        output.write_text(json.dumps(result, indent=2))
        print(output, flush=True)
        if 'idle' in result:
            print(json.dumps({k: v for k, v in result['idle'].items() if k not in ('before', 'after', 'memory_samples')}, indent=2), flush=True)
            print(json.dumps(result['thread_cpu'][:10], indent=2), flush=True)


if __name__ == '__main__':
    main()
