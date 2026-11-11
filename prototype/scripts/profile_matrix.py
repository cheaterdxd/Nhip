#!/usr/bin/env python3
"""Sequential Linux release comparison; never overrides product display defaults.
24 runs x (15s settle + 60s measured). Only processes created here are stopped.
"""
import argparse,itertools,json,os,socket,subprocess,time
from pathlib import Path
from benchmark import ROOT,Client,desktop_run,idle_measure,wait_ready
from idle_diagnostics import threads

def stop(proc):
 if proc and proc.poll() is None:
  proc.terminate()
  try:proc.wait(timeout=5)
  except subprocess.TimeoutExpired:proc.kill();proc.wait()

def main():
 ap=argparse.ArgumentParser();ap.add_argument('--runs',type=int,default=3);ap.add_argument('--seconds',type=int,default=60);ap.add_argument('--settle',type=int,default=15);args=ap.parse_args()
 work=ROOT/'.bench-work'/('matrix-'+time.strftime('%Y%m%d-%H%M%S'));work.mkdir(parents=True)
 output=ROOT/'results'/(work.name+'.json');report={'method':vars(args),'work_dir':str(work),'runs':[],'completed':False}
 backend=None;desktop=None;log=open(work/'backend.log','wb');original=os.environ.copy()
 try:
  with socket.socket() as probe:occupied=probe.connect_ex(('127.0.0.1',4876))==0
  if occupied:raise RuntimeError('Close manual backend first; this matrix uses its own fixture')
  env={**os.environ,'NHIP_DB':str(work/'fixture.sqlite'),'NHIP_DIST':str(ROOT/'dist'),'NHIP_SEED_FIXTURE':'1'};env.pop('NHIP_ACCESS_KEY',None)
  backend=subprocess.Popen([str(ROOT/'backend/target/release/nhip-backend')],env=env,cwd=ROOT,stdout=log,stderr=subprocess.STDOUT)
  report['fixture']=wait_ready(4876,backend)
  for app,display,state,run in itertools.product(['minimal','prototype'],['wayland','x11'],['visible','minimized'],range(args.runs)):
   label=f'{app}-{display}-{state}-{run+1}';print('Starting '+label,flush=True)
   os.environ['GDK_BACKEND']=display;os.environ['NHIP_PROFILE_STATE']=state
   if app=='minimal':os.environ['NHIP_PROFILE_MINIMAL']='1'
   else:os.environ.pop('NHIP_PROFILE_MINIMAL',None)
   desktop,ready=desktop_run(ROOT/'src-tauri/target/release/nhip-desktop',work,label,stay=True)
   if app=='minimal' and not ready['client'].get('minimal'):raise RuntimeError('Minimal page did not load')
   time.sleep(args.settle)
   before=threads(desktop.pid);start=time.monotonic();idle=idle_measure(backend.pid,desktop.pid,args.seconds);elapsed=time.monotonic()-start;after=threads(desktop.pid)
   cpu=sorted([{'tid':tid,'name':v['name'],'cpu_percent_one_core':(v['ticks']-before[tid]['ticks'])/os.sysconf('SC_CLK_TCK')/elapsed*100} for tid,v in after.items() if tid in before],key=lambda v:-v['cpu_percent_one_core'])
   record={'app':app,'display':display,'state_requested':state,'run':run+1,'ready':ready,'idle':idle,'thread_cpu':cpu}
   report['runs'].append(record);output.write_text(json.dumps(report,indent=2));print(label+' CPU='+str(round(idle['desktop_tree']['cpu_percent_of_one_core'],3)),flush=True)
   stop(desktop);desktop=None
  report['completed']=True
 finally:
  stop(desktop);stop(backend);log.close();os.environ.clear();os.environ.update(original);output.write_text(json.dumps(report,indent=2));print('Results: '+str(output),flush=True)
if __name__=='__main__':main()
