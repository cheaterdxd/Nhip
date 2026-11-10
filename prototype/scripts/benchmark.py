#!/usr/bin/env python3
"""Measure release binaries, actual HTTP+SQLite operations and native WebView renders.
No external Python dependencies. Never interprets an unmeasured result as a pass.
"""
import argparse
import concurrent.futures
import http.client
import json
import math
import os
from pathlib import Path
import platform
import signal
import socket
import statistics
import subprocess
import time

ROOT = Path(__file__).resolve().parents[1]


def stats(values):
    ordered = sorted(values)
    if not ordered:
        return None
    return {"samples": len(values), "median_ms": statistics.median(values),
            "p95_ms": ordered[max(0, math.ceil(len(ordered) * .95) - 1)],
            "min_ms": ordered[0], "max_ms": ordered[-1], "raw_ms": values}


class Client:
    def __init__(self, port):
        self.conn = http.client.HTTPConnection("127.0.0.1", port, timeout=15)

    def call(self, method, path, body=None, expected=200):
        data = json.dumps(body, ensure_ascii=False).encode() if body is not None else None
        start = time.perf_counter_ns()
        self.conn.request(method, path, body=data,
                          headers={"content-type": "application/json"} if data else {})
        response = self.conn.getresponse()
        raw = response.read()
        elapsed = (time.perf_counter_ns() - start) / 1e6
        if response.status != expected:
            raise RuntimeError(f"{method} {path}: HTTP {response.status}: {raw[:300]!r}")
        return elapsed, json.loads(raw)

    def close(self):
        self.conn.close()


def wait_ready(port, proc, timeout=30):
    start = time.monotonic()
    while time.monotonic() - start < timeout:
        if proc.poll() is not None:
            raise RuntimeError(f"Backend exited with {proc.returncode}")
        c = Client(port)
        try:
            _, result = c.call("GET", "/api/health")
            return result
        except (ConnectionError, OSError):
            time.sleep(.02)
        finally:
            c.close()
    raise TimeoutError("Backend did not become ready")


def process_table():
    table = {}
    for path in Path('/proc').iterdir():
        if not path.name.isdigit():
            continue
        try:
            text = (path/'stat').read_text()
            data = text[text.rfind(')')+2:].split()
            table[int(path.name)] = (int(data[1]), int(data[11])+int(data[12]))
        except (OSError, ValueError, IndexError):
            continue
    return table


def tree_snapshot(root_pid):
    table = process_table()
    pids = {root_pid}
    while True:
        new = {pid for pid, (parent, _) in table.items() if parent in pids}
        if new.issubset(pids):
            break
        pids |= new
    pss, rss, missing, commands = 0, 0, [], []
    for pid in sorted(pids):
        try:
            memory = {}
            for line in (Path('/proc')/str(pid)/'smaps_rollup').read_text().splitlines():
                key, _, value = line.partition(':')
                if key in ('Pss', 'Rss'):
                    memory[key] = int(value.split()[0])
            pss += memory.get('Pss', 0)
            rss += memory.get('Rss', 0)
            commands.append({'pid': pid, 'command': (Path('/proc')/str(pid)/'comm').read_text().strip()})
        except (OSError, ValueError):
            missing.append(pid)
    return {'pids': sorted(pids), 'pss_kib': pss, 'rss_sum_kib': rss,
            'unreadable_pids': missing, 'processes': commands,
            'ticks': {str(pid): table[pid][1] for pid in pids if pid in table}}


def idle_measure(backend_pid, desktop_pid, seconds):
    roots = {'backend': backend_pid}
    if desktop_pid:
        roots['desktop_tree'] = desktop_pid
    before = {name: tree_snapshot(pid) for name, pid in roots.items()}
    start = time.monotonic()
    samples = {name: [] for name in roots}
    while time.monotonic() - start < seconds:
        time.sleep(min(1, max(.01, seconds-(time.monotonic()-start))))
        for name, pid in roots.items():
            samples[name].append(tree_snapshot(pid))
    duration = time.monotonic()-start
    after = {name: tree_snapshot(pid) for name, pid in roots.items()}
    result = {}
    for name in roots:
        delta = sum(ticks-before[name]['ticks'].get(pid, ticks)
                    for pid, ticks in after[name]['ticks'].items())
        result[name] = {'interval_seconds': duration,
            'cpu_percent_of_one_core': delta/os.sysconf('SC_CLK_TCK')/duration*100,
            'pss_mib_median': statistics.median(s['pss_kib']/1024 for s in samples[name]),
            'pss_mib_max': max(s['pss_kib']/1024 for s in samples[name]),
            'full_tree_pss_available': not any(s['unreadable_pids'] for s in samples[name]),
            'before': before[name], 'after': after[name],
            'memory_samples': [{k:v for k,v in s.items() if k not in ('ticks','processes')} for s in samples[name]]}
    return result


def benchmark_api(port):
    c=Client(port)
    output={}
    def reads(name,path,n):
        for _ in range(15): c.call('GET',path)
        values=[c.call('GET',path)[0] for _ in range(n)]
        output[name]=stats(values)
    reads('today_read','/api/tasks?date=2026-11-09&limit=100',1000)
    reads('week_calendar','/api/calendar?start=2026-11-09',500)
    reads('slot_search_busy_week','/api/slots?task_id=9501&start=2026-11-09',500)
    reads('slot_search_available_week','/api/slots?task_id=9501&start=2027-01-04',500)
    reads('aggregate_report','/api/report',100)
    creates, completes=[],[]
    for i in range(500):
        elapsed,t=c.call('POST','/api/tasks',{'project_id':1,'title':f'Benchmark ghi thật {i}','duration':25,'notes':'Dữ liệu kiểm thử','energy':'medium'})
        creates.append(elapsed)
        elapsed,done=c.call('PATCH',f"/api/tasks/{t['id']}/complete",{'expected_version':t['version']})
        if done['status']!='done': raise AssertionError('Completion did not persist')
        completes.append(elapsed)
    output['create_durable_sqlite']=stats(creates)
    output['complete_durable_sqlite']=stats(completes)
    # Verify optimistic version protection rather than only timing successful no-ops.
    c.call('PATCH',f"/api/tasks/{t['id']}/complete",{'expected_version':t['version']},expected=409)
    output['stale_write_rejected']=True
    c.close()
    def worker(index):
        client=Client(port);values=[]
        try:
            for i in range(50):
                if i%2==0:
                    values.append(client.call('GET','/api/tasks?date=2026-11-09&limit=100')[0])
                    values.append(client.call('GET','/api/calendar?start=2026-11-09')[0])
                else:
                    elapsed,t=client.call('POST','/api/tasks',{'project_id':2,'title':f'Client {index} việc {i}','duration':25})
                    values.append(elapsed)
                    values.append(client.call('PATCH',f"/api/tasks/{t['id']}/complete",{'expected_version':0})[0])
            return values
        finally: client.close()
    with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:
        values=[v for group in pool.map(worker,range(4)) for v in group]
    output['four_clients_mixed_400_requests']=stats(values)
    return output


def desktop_run(binary, work, index, mode='startup', stay=False):
    output=work/f'desktop-{index}.json'
    env={**os.environ,'NHIP_BENCH_OUTPUT':str(output),'NHIP_BENCH_MODE':mode}
    if stay: env['NHIP_BENCH_STAY_OPEN']='1'
    logfile=open(work/f'desktop-{index}.log','wb')
    start=time.perf_counter()
    proc=subprocess.Popen([str(binary)],env=env,stdout=logfile,stderr=subprocess.STDOUT)
    try:
        deadline=time.monotonic()+40
        while not output.exists():
            if proc.poll() is not None: raise RuntimeError(f'Desktop exited {proc.returncode}; inspect {logfile.name}')
            if time.monotonic()>deadline: raise TimeoutError(f'Desktop did not report; inspect {logfile.name}')
            time.sleep(.01)
        report=json.loads(output.read_text())
        report['external_spawn_to_output_ms']=(time.perf_counter()-start)*1000
        if report['client'].get('error'): raise RuntimeError(str(report['client']))
        if not stay: proc.wait(timeout=10)
        return proc,report
    except Exception:
        proc.terminate()
        try:proc.wait(timeout=5)
        except subprocess.TimeoutExpired:proc.kill();proc.wait()
        raise
    finally:logfile.close()


def main():
    parser=argparse.ArgumentParser()
    parser.add_argument('--backend',type=Path,default=ROOT/'backend/target/release/nhip-backend')
    parser.add_argument('--desktop',type=Path,default=ROOT/'src-tauri/target/release/nhip-desktop')
    parser.add_argument('--desktop-runs',type=int,default=20)
    parser.add_argument('--idle-seconds',type=int,default=60)
    parser.add_argument('--skip-desktop',action='store_true')
    parser.add_argument('--output',type=Path,default=ROOT/'results/latest.json')
    args=parser.parse_args()
    with socket.socket() as probe:
        if probe.connect_ex(('127.0.0.1',4876)) == 0:
            raise RuntimeError('Port 4876 is in use. Stop the manual prototype backend first; refusing to benchmark someone else\'s database.')
    work=ROOT/'.bench-work'/time.strftime('%Y%m%d-%H%M%S')
    work.mkdir(parents=True,exist_ok=True)
    env={**os.environ,'NHIP_DB':str(work/'fixture.sqlite'),'NHIP_DIST':str(ROOT/'dist'),'NHIP_PORT':'4876'}
    log=open(work/'backend.log','wb')
    backend=subprocess.Popen([str(args.backend.resolve())],env=env,cwd=ROOT,stdout=log,stderr=subprocess.STDOUT)
    desktop=None
    result={'method':{'build':'release','network':'loopback 127.0.0.1; no Tailscale','dataset':'100 projects / 10,000 initial occurrences; benchmark appends real writes','database':'SQLite WAL, synchronous FULL','ui':'synthetic state-to-two-animation-frames; NOT real-user INP','startup':'fresh process; warm filesystem/OS/WebView caches, not cold boot','idle':'Linux process tree PSS plus CPU ticks / one CPU core'},'environment':{'platform':platform.platform(),'cpu_count':os.cpu_count(),'display':os.getenv('DISPLAY'),'wayland_display':os.getenv('WAYLAND_DISPLAY')},'work_dir':str(work)}
    try:
        result['fixture_before']=wait_ready(4876,backend)
        print('Backend ready; measuring real HTTP + SQLite operations.',flush=True)
        result['api']=benchmark_api(4876)
        if not args.skip_desktop:
            startup=[]
            for i in range(args.desktop_runs):
                print(f'Desktop startup {i+1}/{args.desktop_runs}',flush=True)
                _,report=desktop_run(args.desktop.resolve(),work,str(i))
                startup.append(report)
            result['desktop_startup']={'process_start_to_ready':stats([r['process_start_to_result_ms'] for r in startup]),'raw_runs':startup}
            print('Measuring native WebView interaction renders.',flush=True)
            _,result['desktop_interactions']=desktop_run(args.desktop.resolve(),work,'interactions',mode='interactions')
            desktop,result['desktop_idle_ready']=desktop_run(args.desktop.resolve(),work,'idle',stay=True)
        print(f'Measuring idle CPU and memory for {args.idle_seconds} seconds.',flush=True)
        result['idle']=idle_measure(backend.pid,desktop.pid if desktop else None,args.idle_seconds)
        c=Client(4876);_,result['fixture_after']=c.call('GET','/api/health');c.close()
        result['completed']=True
    except Exception as error:
        result['completed']=False;result['error']=str(error)
        raise
    finally:
        for proc in (desktop,backend):
            if proc and proc.poll() is None:
                proc.terminate()
                try:proc.wait(timeout=5)
                except subprocess.TimeoutExpired:proc.kill();proc.wait()
        log.close()
        args.output.parent.mkdir(parents=True,exist_ok=True)
        args.output.write_text(json.dumps(result,ensure_ascii=False,indent=2))
        print(f'Raw results: {args.output}',flush=True)

if __name__=='__main__':main()
