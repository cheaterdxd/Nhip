#!/usr/bin/env python3
"""Smoke-test app data against a new disposable database, without sample data."""
import json, os, socket, sqlite3, subprocess, tempfile, time, urllib.error, urllib.request
from datetime import datetime, timezone, timedelta
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
def main():
 with socket.socket() as sock:
  sock.bind(('127.0.0.1',0));port=sock.getsockname()[1]
 with tempfile.TemporaryDirectory(prefix='nhip-product-') as folder:
  env={**os.environ,'NHIP_DB':str(Path(folder)/'app.sqlite'),'NHIP_DIST':str(ROOT/'dist'),'NHIP_PORT':str(port),'NHIP_ACCESS_KEY':'test-only','NHIP_SEED_FIXTURE':'0'}
  process=None
  def request(path,data=None,method=None):
   headers={'authorization':'Bearer test-only'}
   if data is not None:headers['content-type']='application/json';data=json.dumps(data,ensure_ascii=False).encode()
   req=urllib.request.Request(f'http://127.0.0.1:{port}'+path,data=data,headers=headers,method=method)
   try:
    with urllib.request.urlopen(req,timeout=5) as res:return res.status,json.load(res)
   except urllib.error.HTTPError as e:return e.code,e.read().decode()
  def start():
   p=subprocess.Popen([str(ROOT/'backend/target/release/nhip-backend')],cwd=ROOT,env=env,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
   for _ in range(200):
    if p.poll() is not None:raise RuntimeError('Backend exited')
    try:
     if request('/api/health')[0]==200:return p
    except OSError:time.sleep(.05)
   p.terminate();p.wait();raise TimeoutError('Backend did not start')
  def stop(p):
   if p and p.poll() is None:p.terminate();p.wait(timeout=5)
  try:
   process=start()
   status,health=request('/api/health');assert status==200 and health['projects']==0 and health['occurrences']==0
   assert request('/api/app/projects')==(200,[])
   project_payload={'device_id':'device-a','request_id':'project-create-1','name':'Dự án cá nhân','area':'personal','description':'Bối cảnh'}
   status,project=request('/api/app/projects',project_payload);assert status==200
   assert request('/api/app/projects',project_payload)==(status,project)
   assert request('/api/app/projects',{**project_payload,'name':'Khác'})[0]==409
   payload={'device_id':'device-a','request_id':'capture-1','kind':'task','content':'Việc cần nhớ','project_id':None,'duration':None,'energy':None}
   created=request('/api/app/items',payload);assert created[0]==200 and created[1]['status']=='inbox'
   assert request('/api/app/items',payload)==created
   assert request('/api/app/items',{**payload,'content':'nội dung khác'})[0]==409
   text='  Note nguyên văn\nDòng thứ hai  '
   note=request('/api/app/items',{**payload,'request_id':'capture-2','kind':'note','content':text,'project_id':project['id']});assert note[1]['content']==text
   note_id=note[1]['id']; edit={'device_id':'device-a','request_id':'edit-note-1','expected_version':0,'content':'Nội dung đã sửa\nDòng mới'}
   edited=request(f'/api/app/items/{note_id}/content',edit)
   assert edited[0]==200 and edited[1]['content']==edit['content'] and edited[1]['original_content']==text
   assert request(f'/api/app/items/{note_id}/content',edit)==edited
   assert request(f'/api/app/items/{note_id}/content',{**edit,'request_id':'stale-note','content':'ghi đè sai'})[0]==409
   assert request(f'/api/app/items/{note_id}/content',{**edit,'content':'nội dung khác'})[0]==409
   assert request('/api/app/items',{**payload,'request_id':'capture-3','project_id':999999})[0]==400
   item_id=created[1]['id']
   clarification={'device_id':'device-a','request_id':'clarify-1','expected_version':0,'project_id':project['id'],'duration':45,'energy':'high'}
   # request() normally infers POST from a body, matching the clarify endpoint.
   result=request(f'/api/app/items/{item_id}/clarify',clarification)
   assert result[0]==200 and result[1]['project_id']==project['id'] and result[1]['version']==1 and result[1]['duration']==45
   assert request(f'/api/app/items/{item_id}/clarify',clarification)==result
   assert request(f'/api/app/items/{item_id}/clarify',{**clarification,'request_id':'other-device-write','duration':30})[0]==409
   assert request(f'/api/app/items/{item_id}/clarify',{**clarification,'project_id':999999})[0]==409
   project_update={'device_id':'device-a','request_id':'project-pause','expected_version':0,'name':'Dự án đã sửa','description':'Mô tả mới','status':'paused'}
   paused=request(f"/api/app/projects/{project['id']}",project_update,'PATCH')
   assert paused[0]==200 and paused[1]['status']=='paused' and paused[1]['open_task_count']==1 and paused[1]['version']==1
   assert request(f"/api/app/projects/{project['id']}",project_update,'PATCH')==paused
   assert request(f"/api/app/projects/{project['id']}",{**project_update,'request_id':'old-project-write','status':'active'},'PATCH')[0]==409
   close={**project_update,'request_id':'close-project','expected_version':1,'status':'completed'}
   assert request(f"/api/app/projects/{project['id']}",close,'PATCH')[0]==409
   reactivate={**project_update,'request_id':'reactivate-project','expected_version':1,'status':'active'}
   assert request(f"/api/app/projects/{project['id']}",reactivate,'PATCH')[1]['version']==2
   action={'device_id':'device-a','request_id':'finish-1','expected_version':1,'action':'complete','reason':None}
   completed=request(f'/api/app/items/{item_id}/transition',action)
   assert completed[0]==200 and completed[1]['status']=='done' and completed[1]['version']==2
   assert request(f'/api/app/items/{item_id}/transition',action)==completed
   assert request(f'/api/app/items/{item_id}/transition',{**action,'request_id':'stale','action':'abandon','reason':'đã đổi'})[0]==409
   undo={**action,'request_id':'undo-1','expected_version':2,'action':'undo'}
   restored=request(f'/api/app/items/{item_id}/transition',undo)
   assert restored[0]==200 and restored[1]['status']=='ready' and restored[1]['version']==3
   abandon={**action,'request_id':'abandon-1','expected_version':3,'action':'abandon','reason':'Không còn phù hợp'}
   assert request(f'/api/app/items/{item_id}/transition',{**abandon,'reason':'  '})[0]==400
   abandoned=request(f'/api/app/items/{item_id}/transition',abandon)
   assert abandoned[0]==200 and abandoned[1]['status']=='abandoned'
   close={**close,'request_id':'close-after-abandon','expected_version':2}
   closed=request(f"/api/app/projects/{project['id']}",close,'PATCH')
   assert closed[0]==200 and closed[1]['status']=='completed' and closed[1]['open_task_count']==0
   assert request(f'/api/app/items/{item_id}/transition',{**undo,'request_id':'restore-closed','expected_version':4})[0]==409
   status,history=request(f'/api/app/items/{item_id}/history')
   assert status==200 and history[0]['action']=='abandon' and history[0]['after']['reason']=='Không còn phù hợp'
   stop(process);process=start()
   assert request('/api/app/items',payload)==created
   assert request(f'/api/app/items/{item_id}/clarify',clarification)==result
   assert request(f'/api/app/items/{note_id}/content',edit)==edited
   status,items=request('/api/app/items');assert status==200 and len(items)==2 and any(i['content']==edit['content'] and i['original_content']==text for i in items)
   assert any(i['id']==item_id and i['project_id']==project['id'] and i['duration']==45 for i in items)
   stop(process)
   legacy=Path(folder)/'legacy.sqlite'
   with sqlite3.connect(legacy) as db:
    db.execute("CREATE TABLE app_items(id INTEGER PRIMARY KEY,kind TEXT NOT NULL,content TEXT NOT NULL,project_id INTEGER,status TEXT NOT NULL DEFAULT 'inbox',duration INTEGER,energy TEXT,version INTEGER NOT NULL DEFAULT 0,created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP)")
    db.execute("INSERT INTO app_items(kind,content) VALUES('note',?)",('Ghi chú cũ',))
   env['NHIP_DB']=str(legacy);process=start()
   status,migrated=request('/api/app/items');assert status==200 and migrated[0]['content']=='Ghi chú cũ' and migrated[0]['original_content']=='Ghi chú cũ'
   recurrence_task=request('/api/app/items',{**payload,'request_id':'recurrence-template','content':'Uống thuốc'})[1]
   first_day=(datetime.now(timezone.utc)+timedelta(hours=7)).date()
   while first_day.weekday()>=5:first_day+=timedelta(days=1)
   rule_payload={'device_id':'device-a','request_id':'recurrence-create','expected_task_version':recurrence_task['version'],'task_id':recurrence_task['id'],'frequency':'daily','weekdays':[],'month_day':None,'local_time':'23:59','starts_on':first_day.isoformat()}
   rule=request('/api/app/recurring-rules',rule_payload);assert rule[0]==200 and rule[1]['status']=='active'
   assert request('/api/app/recurring-rules',rule_payload)==rule
   assert request('/api/app/recurring-rules')[1][0]['template_task_id']==recurrence_task['id']
   status,calendar=request(f"/api/app/calendar?start={first_day.isoformat()}&days=1")
   assert status==200 and any(item['task_id']==recurrence_task['id'] for item in calendar['appointments'])
   pause={'device_id':'device-a','request_id':'recurrence-pause','expected_version':rule[1]['version'],'status':'paused'}
   paused=request(f"/api/app/recurring-rules/{rule[1]['id']}",pause,'PATCH');assert paused[0]==200 and paused[1]['status']=='paused'
   resume={**pause,'request_id':'recurrence-resume','expected_version':paused[1]['version'],'status':'active'}
   assert request(f"/api/app/recurring-rules/{rule[1]['id']}",resume,'PATCH')[1]['status']=='active'
   task=request('/api/app/items',{**payload,'request_id':'reminder-task','content':'Nhắc việc thử'})[1]
   assert request('/api/devices',{'id':'desktop-a','channel':'desktop'})[0]==200
   now=int(time.time())
   schedule={'device_id':'device-a','request_id':'schedule-1','task_id':task['id'],'expected_task_version':task['version'],'due_at':now+3600}
   created_reminder=request('/api/app/reminders',schedule);assert created_reminder[0]==200
   assert request('/api/app/reminders',schedule)==created_reminder
   rid=created_reminder[1]['id'];version=created_reminder[1]['version']
   future={'device_id':'device-a','request_id':'move-before','expected_version':version,'due_at':now+7200,'reason':None}
   moved=request(f'/api/app/reminders/{rid}/reschedule',future);assert moved[0]==200 and moved[1]['snoozes']==0
   for count in range(1,4):
    # Advance the commitment clock without sleeping through three appointments.
    with sqlite3.connect(legacy) as db:db.execute('UPDATE app_task_reminders SET due_at=?,next_fire=? WHERE id=?',(now-1,0 if count==1 else now+86400,rid))
    if count==1:
     for _ in range(40):
      status,updates=request('/api/devices/desktop-a/updates')
      app_alerts=[r for r in updates['reminders'] if r.get('source')=='app' and r['task_id']==task['id']]
      if app_alerts:break
      time.sleep(.1)
     assert app_alerts and app_alerts[0]['ack_device'] is None
    version=request('/api/app/reminders')[1][0]['version']
    postpone={'device_id':'device-a','request_id':f'postpone-{count}','expected_version':version,'due_at':now+3600+count,'reason':'Bận việc'}
    assert request(f'/api/app/reminders/{rid}/reschedule',{**postpone,'reason':None})[0]==400
    result=request(f'/api/app/reminders/{rid}/reschedule',postpone);assert result[0]==200 and result[1]['snoozes']==count and result[1]['state']=='scheduled'
    assert request(f'/api/app/reminders/{rid}/reschedule',postpone)==result
    if count==1:
     _,updates=request('/api/devices/desktop-a/updates')
     assert next(r for r in updates['reminders'] if r['id']==app_alerts[0]['id'])['ack_device']=='resolved'
   with sqlite3.connect(legacy) as db:db.execute('UPDATE app_task_reminders SET due_at=?,next_fire=? WHERE id=?',(now-1,now+86400,rid))
   latest=request('/api/app/reminders')[1][0]
   assert request(f'/api/app/reminders/{rid}/reschedule',{**postpone,'request_id':'fourth','expected_version':latest['version']})[0]==409
   with sqlite3.connect(legacy) as db:db.execute('UPDATE app_task_reminders SET next_fire=0 WHERE id=?',(rid,))
   for _ in range(40):
    latest=request('/api/app/reminders')[1][0]
    if latest['state']=='decision':break
    time.sleep(.1)
   assert latest['state']=='decision' and latest['snoozes']==3
   _,updates=request('/api/devices/desktop-a/updates')
   decision_alert=next(r for r in updates['reminders'] if r.get('source')=='app' and r.get('kind')=='decision' and r['task_id']==task['id'])
   assert decision_alert['ack_device'] is None
   stop(process);process=start();assert request('/api/app/reminders')[1][0]['state']=='decision'
   assert next(r for r in request('/api/app/recurring-rules')[1] if r['id']==rule[1]['id'])['status']=='active'
   current_task=next(i for i in request('/api/app/items')[1] if i['id']==task['id'])
   start_focus={'device_id':'device-a','request_id':'focus-start','expected_version':latest['version'],'planned_seconds':60}
   focus=request(f'/api/app/reminders/{rid}/start-focus',start_focus);assert focus[0]==200 and focus[1]['state']=='active'
   assert request(f'/api/app/reminders/{rid}/start-focus',start_focus)==focus
   assert request('/api/app/focus-sessions')[1][0]['id']==focus[1]['id']
   time.sleep(1.05)
   finish_focus={'device_id':'device-a','request_id':'focus-finish','expected_version':0}
   finished=request(f"/api/app/focus-sessions/{focus[1]['id']}/finish",finish_focus)
   assert finished[0]==200 and finished[1]['state']=='interrupted' and finished[1]['actual_seconds']>=1
   assert request('/api/app/focus-sessions')[1]==[] and request('/api/app/reminders')[1][0]['state']=='decision'
   current_task=next(i for i in request('/api/app/items')[1] if i['id']==task['id'])
   discard={'device_id':'device-a','request_id':'decision-abandon','expected_version':current_task['version'],'action':'abandon','reason':'Không còn cần thiết'}
   discarded=request(f"/api/app/items/{task['id']}/transition",discard);assert discarded[0]==200
   assert next(r for r in request('/api/app/reminders')[1] if r['id']==rid)['state']=='closed'
   restore={**discard,'request_id':'decision-undo','expected_version':discarded[1]['version'],'action':'undo','reason':None}
   restored=request(f"/api/app/items/{task['id']}/transition",restore);assert restored[1]['status']=='decision'
   assert request('/api/app/reminders')[1][0]['state']=='decision'
   replacement={'device_id':'device-a','request_id':'decision-restructure','expected_version':restored[1]['version'],'content':'Gửi email cho khách hàng về bản dự thảo','reason':'Task gốc quá mơ hồ, cần một bước thực hiện cụ thể','duration':20,'energy':'medium','due_at':now+7200}
   repl=request(f'/api/app/reminders/{rid}/restructure',replacement)
   current_state=next(i for i in request('/api/app/items')[1] if i['id']==task['id'])
   assert repl[0]==200 and repl[1]['original']['status']=='restructured' and repl[1]['replacement']['status']=='ready', {'response':repl,'restored':restored,'current':current_state}
   assert repl[1]['replacement']['content']==replacement['content'] and repl[1]['reminder']['snoozes']==0
   assert request(f'/api/app/reminders/{rid}/restructure',replacement)==repl
   with sqlite3.connect(legacy) as db:
    assert db.execute('SELECT parent_task_id,child_task_id,reason FROM app_task_relations').fetchone()==(task['id'],repl[1]['replacement']['id'],replacement['reason'])
   report_task=request('/api/app/items',{**payload,'request_id':'weekly-report-task','content':'Việc hoàn thành trong tuần'})[1]
   report_complete={'device_id':'device-a','request_id':'weekly-report-complete','expected_version':report_task['version'],'action':'complete','reason':None}
   assert request(f"/api/app/items/{report_task['id']}/transition",report_complete)[1]['status']=='done'
   local_today=(datetime.now(timezone.utc)+timedelta(hours=7)).date()
   week_start=local_today-timedelta(days=local_today.weekday())
   status,weekly=request(f'/api/app/reports/weekly?week_start={week_start.isoformat()}')
   assert status==200 and weekly['counts']['postponements']==3 and len(weekly['postponements'])==3
   assert all(row['reason']=='Bận việc' for row in weekly['postponements']) and weekly['counts']['completed']>=1, weekly
   assert weekly['ai_analysis_available'] is True
   assert request('/api/app/reports/weekly?week_start=2026-11-10')[0]==400
   assert request('/api/devices',{'id':'browser-b','channel':'browser'})[0]==200
   with sqlite3.connect(legacy) as db:db.execute('INSERT INTO app_weekly_report_alerts(week_start,created_at) VALUES(?,?)',(week_start.isoformat(),now))
   _,updates=request('/api/devices/desktop-a/updates')
   report_notice=next(row for row in updates['reminders'] if row.get('source')=='report' and row['week_start']==week_start.isoformat())
   iso=week_start.isocalendar();expected_report_id=1_500_000_000+iso.year*100+iso.week
   assert report_notice['ack_device'] is None and report_notice['id']==expected_report_id
   report_ack={'device_id':'browser-b'}
   assert request(f"/api/app/reports/weekly/{week_start.isoformat()}/ack",report_ack)[1]['ack_device']=='browser-b'
   assert request(f"/api/app/reports/weekly/{week_start.isoformat()}/ack",report_ack)[1]['ack_device']=='browser-b'
   _,updates=request('/api/devices/desktop-a/updates')
   assert next(row for row in updates['reminders'] if row.get('source')=='report' and row['week_start']==week_start.isoformat())['ack_device']=='browser-b'
   stop(process);process=start()
   assert next(r for r in request('/api/app/reminders')[1] if r['id']==rid)['state']=='closed'
   assert any(i['id']==repl[1]['replacement']['id'] for i in request('/api/app/items')[1])
   print('PASS: app data, migration, recurring rules, three snoozes, decision alert, focus, restructuring, idempotency, restart')
  finally:stop(process)
if __name__=='__main__':main()
