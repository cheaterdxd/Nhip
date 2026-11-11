//! Durable reminder commitments for real app tasks, delivered through registered devices.
use super::*;
use crate::product::get_item;

pub fn schema(c:&Connection)->rusqlite::Result<()> {c.execute_batch("CREATE TABLE IF NOT EXISTS app_task_reminders(
 id INTEGER PRIMARY KEY,task_id INTEGER NOT NULL UNIQUE REFERENCES app_items(id),due_at INTEGER NOT NULL,next_fire INTEGER,
 snoozes INTEGER NOT NULL DEFAULT 0 CHECK(snoozes BETWEEN 0 AND 3),reminder_index INTEGER NOT NULL DEFAULT 0,
 state TEXT NOT NULL DEFAULT 'scheduled' CHECK(state IN ('scheduled','decision','closed')),closed_from TEXT,version INTEGER NOT NULL DEFAULT 0);
 CREATE INDEX IF NOT EXISTS app_task_reminders_due ON app_task_reminders(state,next_fire);
 CREATE TABLE IF NOT EXISTS app_reminder_events(id INTEGER PRIMARY KEY,reminder_id INTEGER NOT NULL REFERENCES app_task_reminders(id),kind TEXT NOT NULL,at INTEGER NOT NULL,device_id TEXT,reason TEXT,due_at INTEGER);
 CREATE TABLE IF NOT EXISTS app_reminder_alerts(id INTEGER PRIMARY KEY,reminder_id INTEGER NOT NULL REFERENCES app_task_reminders(id),created_at INTEGER NOT NULL,kind TEXT NOT NULL DEFAULT 'reminder');
 CREATE TABLE IF NOT EXISTS app_push_delivery(alert_id INTEGER NOT NULL REFERENCES app_reminder_alerts(id),device TEXT NOT NULL,attempts INTEGER NOT NULL DEFAULT 0,next_try INTEGER NOT NULL DEFAULT 0,status TEXT NOT NULL DEFAULT 'pending',PRIMARY KEY(alert_id,device));
 CREATE TABLE IF NOT EXISTS app_focus_sessions(id INTEGER PRIMARY KEY,reminder_id INTEGER REFERENCES app_task_reminders(id),task_id INTEGER NOT NULL REFERENCES app_items(id),started_at INTEGER NOT NULL,planned_seconds INTEGER NOT NULL CHECK(planned_seconds BETWEEN 60 AND 5400),ended_at INTEGER,actual_seconds INTEGER,state TEXT NOT NULL CHECK(state IN ('active','finished','interrupted')),version INTEGER NOT NULL DEFAULT 0);
 CREATE UNIQUE INDEX IF NOT EXISTS app_one_active_focus ON app_focus_sessions(state) WHERE state='active';
 CREATE TABLE IF NOT EXISTS app_task_relations(parent_task_id INTEGER NOT NULL REFERENCES app_items(id),child_task_id INTEGER NOT NULL UNIQUE REFERENCES app_items(id),relation TEXT NOT NULL,reason TEXT NOT NULL,created_at INTEGER NOT NULL,PRIMARY KEY(parent_task_id,child_task_id));")?;
 let has_reset={let mut q=c.prepare("PRAGMA table_info(app_task_reminders)")?;let cols=q.query_map([],|r|r.get::<_,String>(1))?.collect::<Result<Vec<_>,_>>()?;cols.iter().any(|v|v=="last_reset_alert")};
 if !has_reset{c.execute_batch("ALTER TABLE app_task_reminders ADD COLUMN last_reset_alert INTEGER NOT NULL DEFAULT 0;")?;}
 let focus_requires_migration={let mut q=c.prepare("PRAGMA table_info(app_focus_sessions)")?;let columns=q.query_map([],|r|Ok((r.get::<_,String>(1)?,r.get::<_,i64>(3)?)))?.collect::<Result<Vec<_>,_>>()?;columns.iter().any(|(name,required)|name=="reminder_id"&&*required==1)};
 if focus_requires_migration{c.execute_batch("BEGIN IMMEDIATE;
 CREATE TABLE app_focus_sessions_new(id INTEGER PRIMARY KEY,reminder_id INTEGER REFERENCES app_task_reminders(id),task_id INTEGER NOT NULL REFERENCES app_items(id),started_at INTEGER NOT NULL,planned_seconds INTEGER NOT NULL CHECK(planned_seconds BETWEEN 60 AND 5400),ended_at INTEGER,actual_seconds INTEGER,state TEXT NOT NULL CHECK(state IN ('active','finished','interrupted')),version INTEGER NOT NULL DEFAULT 0);
 INSERT INTO app_focus_sessions_new SELECT id,reminder_id,task_id,started_at,planned_seconds,ended_at,actual_seconds,state,version FROM app_focus_sessions;
 DROP TABLE app_focus_sessions;
 ALTER TABLE app_focus_sessions_new RENAME TO app_focus_sessions;
 CREATE UNIQUE INDEX app_one_active_focus ON app_focus_sessions(state) WHERE state='active';
 COMMIT;")?;}
 Ok(())}
fn invalid(s:&str)->ApiError{ApiError(StatusCode::BAD_REQUEST,s.into())}
fn conflict(s:&str)->ApiError{ApiError(StatusCode::CONFLICT,s.into())}
fn get(c:&Connection,id:i64)->Result<Value,ApiError>{
 c.query_row("SELECT r.id,r.task_id,r.due_at,r.next_fire,r.snoozes,r.reminder_index,r.state,r.version,i.content FROM app_task_reminders r JOIN app_items i ON i.id=r.task_id WHERE r.id=?1",[id],|r|Ok(json!({"id":r.get::<_,i64>(0)?,"task_id":r.get::<_,i64>(1)?,"due_at":r.get::<_,i64>(2)?,"next_fire":r.get::<_,Option<i64>>(3)?,"snoozes":r.get::<_,i64>(4)?,"reminder_index":r.get::<_,i64>(5)?,"state":r.get::<_,String>(6)?,"version":r.get::<_,i64>(7)?,"title":r.get::<_,String>(8)?}))).optional()?.ok_or_else(||ApiError(StatusCode::NOT_FOUND,"Reminder not found".into()))
}
fn valid_identity(device:&str,request:&str)->bool{!device.is_empty()&&device.len()<=100&&!request.is_empty()&&request.len()<=100}
fn ensure_no_calendar_conflict(c:&Connection,task_id:i64,due_at:i64,ignore_reminder:Option<i64>)->Result<(),ApiError>{
 let duration:i64=c.query_row("SELECT coalesce(duration,25) FROM app_items WHERE id=?1",[task_id],|r|r.get(0))?;
 let start=due_at-600;let end=due_at+duration*60+600;
 let event:bool=c.query_row("SELECT EXISTS(SELECT 1 FROM app_busy_events WHERE starts_at<?2 AND ends_at>?1)",params![start,end],|r|r.get(0))?;
 let reminder:bool=c.query_row("SELECT EXISTS(SELECT 1 FROM app_task_reminders r JOIN app_items i ON i.id=r.task_id WHERE r.state IN ('scheduled','decision') AND r.id IS NOT coalesce(?1,-1) AND r.due_at<?3 AND r.due_at+coalesce(i.duration,25)*60>?2)",params![ignore_reminder,start,end],|r|r.get(0))?;
 let planned:bool=c.query_row("SELECT EXISTS(SELECT 1 FROM app_schedule_sessions s JOIN app_schedule_plans p ON p.id=s.plan_id WHERE p.task_id<>?1 AND p.state='active' AND s.state='planned' AND s.starts_at<?3 AND s.starts_at+s.duration_minutes*60>?2)",params![task_id,start,end],|r|r.get(0))?;
 let focus:bool=c.query_row("SELECT EXISTS(SELECT 1 FROM app_focus_sessions WHERE state='active' AND started_at<?2 AND started_at+planned_seconds>?1)",params![start,end],|r|r.get(0))?;
 if event||reminder||planned||focus{return Err(conflict("This time overlaps a busy event, another task, or an active focus session; refresh suggestions"))}Ok(())
}
#[derive(Deserialize,Serialize)] pub struct Schedule{device_id:String,request_id:String,task_id:i64,expected_task_version:i64,due_at:i64}
pub async fn schedule(State(s):State<AppState>,Json(input):Json<Schedule>)->ApiResult<Value>{
 if !valid_identity(&input.device_id,&input.request_id)||input.expected_task_version<0||input.due_at<946684800||input.due_at>4102444800{return Err(invalid("Invalid reminder schedule"))}
 let out=db_job(s.clone(),move|c|{
  let canonical=json!({"action":"schedule_reminder","data":input}).to_string();let tx=c.transaction()?;
  let prior:Option<(String,String)>=tx.query_row("SELECT input,result FROM app_commands WHERE device_id=?1 AND request_id=?2",params![input.device_id,input.request_id],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
  if let Some((old,result))=prior{if old!=canonical{return Err(conflict("Request ID already used for different data"))}return serde_json::from_str(&result).map_err(|e|ApiError(StatusCode::INTERNAL_SERVER_ERROR,e.to_string()))}
  let task:Option<(String,String,i64)>=tx.query_row("SELECT kind,status,version FROM app_items WHERE id=?1",[input.task_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional()?;
  let (kind,status,version)=task.ok_or_else(||ApiError(StatusCode::NOT_FOUND,"Task not found".into()))?;
  if kind!="task"||!matches!(status.as_str(),"inbox"|"ready")||version!=input.expected_task_version{return Err(conflict("Task changed or cannot be scheduled"))}
  let exists:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM app_task_reminders WHERE task_id=?1)",[input.task_id],|r|r.get(0))?;
  if exists{return Err(conflict("Task already has a reminder; reschedule the existing commitment"))}
  ensure_no_calendar_conflict(&tx,input.task_id,input.due_at,None)?;
  tx.execute("INSERT INTO app_task_reminders(task_id,due_at,next_fire) VALUES(?1,?2,?2)",params![input.task_id,input.due_at])?;
  let id=tx.last_insert_rowid();
  tx.execute("UPDATE app_items SET version=version+1,updated_at=CURRENT_TIMESTAMP WHERE id=?1 AND version=?2",params![input.task_id,input.expected_task_version])?;
  tx.execute("INSERT INTO app_reminder_events(reminder_id,kind,at,device_id,due_at) VALUES(?1,'schedule',?2,?3,?4)",params![id,chrono::Utc::now().timestamp(),input.device_id,input.due_at])?;
  let result=get(&tx,id)?;
  tx.execute("INSERT INTO app_commands(device_id,request_id,input,result) VALUES(?1,?2,?3,?4)",params![input.device_id,input.request_id,canonical,result.to_string()])?;tx.commit()?;Ok(result)
 }).await?;
 s.revision.send_modify(|v|*v+=1);Ok(out)
}
pub async fn list(State(s):State<AppState>)->ApiResult<Vec<Value>>{db_job(s,|c|{
 let mut q=c.prepare("SELECT id FROM app_task_reminders ORDER BY id DESC")?;
 let ids=q.query_map([],|r|r.get::<_,i64>(0))?.collect::<Result<Vec<_>,_>>()?;
 ids.into_iter().map(|id|get(c,id)).collect()
}).await}
#[derive(Deserialize,Serialize)] pub struct StartFocus{device_id:String,request_id:String,expected_version:i64,planned_seconds:Option<i64>}
pub async fn start_focus(State(s):State<AppState>,Path(id):Path<i64>,Json(input):Json<StartFocus>)->ApiResult<Value>{
 let planned=input.planned_seconds.unwrap_or(1500);
 if !valid_identity(&input.device_id,&input.request_id)||input.expected_version<0||!(60..=5400).contains(&planned){return Err(invalid("Invalid focus session"))}
 let out=db_job(s.clone(),move|c|{
  let canonical=json!({"action":"start_focus","reminder_id":id,"data":input});let tx=c.transaction()?;
  let prior:Option<(String,String)>=tx.query_row("SELECT input,result FROM app_commands WHERE device_id=?1 AND request_id=?2",params![input.device_id,input.request_id],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
  if let Some((old,result))=prior{if old!=canonical.to_string(){return Err(conflict("Request ID already used for different data"))}return serde_json::from_str(&result).map_err(|e|ApiError(StatusCode::INTERNAL_SERVER_ERROR,e.to_string()))}
  let reminder=get(&tx,id)?;
  if reminder["state"]!="decision"||reminder["version"].as_i64()!=Some(input.expected_version){return Err(conflict("Task is not awaiting a decision or changed on another device"))}
  let active:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM app_focus_sessions WHERE state='active')",[],|r|r.get(0))?;
  if active{return Err(conflict("Finish the active focus session before starting another"))}
  let task_id=reminder["task_id"].as_i64().ok_or_else(||invalid("Invalid task"))?;let now=chrono::Utc::now().timestamp();
  tx.execute("INSERT INTO app_focus_sessions(reminder_id,task_id,started_at,planned_seconds,state) VALUES(?1,?2,?3,?4,'active')",params![id,task_id,now,planned])?;
  let session_id=tx.last_insert_rowid();tx.execute("UPDATE app_task_reminders SET last_reset_alert=COALESCE((SELECT MAX(id) FROM app_reminder_alerts WHERE reminder_id=?1),0),version=version+1 WHERE id=?1 AND version=?2",params![id,input.expected_version])?;
  let result=json!({"id":session_id,"reminder_id":id,"task_id":task_id,"started_at":now,"planned_seconds":planned,"state":"active","version":0});
  tx.execute("INSERT INTO app_reminder_events(reminder_id,kind,at,device_id) VALUES(?1,'start_now',?2,?3)",params![id,now,input.device_id])?;
  tx.execute("INSERT INTO app_commands(device_id,request_id,input,result) VALUES(?1,?2,?3,?4)",params![input.device_id,input.request_id,canonical.to_string(),result.to_string()])?;tx.commit()?;Ok(result)
 }).await?;s.revision.send_modify(|v|*v+=1);Ok(out)
}
#[derive(Deserialize,Serialize)] pub struct StartTaskFocus{device_id:String,request_id:String,expected_task_version:i64,planned_seconds:Option<i64>}
pub async fn start_task_focus(State(s):State<AppState>,Path(id):Path<i64>,Json(input):Json<StartTaskFocus>)->ApiResult<Value>{
 let planned=input.planned_seconds.unwrap_or(1500);
 if !valid_identity(&input.device_id,&input.request_id)||input.expected_task_version<0||!(60..=5400).contains(&planned){return Err(invalid("Invalid focus session"))}
 let out=db_job(s.clone(),move|c|{
  let canonical=json!({"action":"start_task_focus","task_id":id,"data":input}).to_string();let tx=c.transaction()?;
  let prior:Option<(String,String)>=tx.query_row("SELECT input,result FROM app_commands WHERE device_id=?1 AND request_id=?2",params![input.device_id,input.request_id],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
  if let Some((old,result))=prior{if old!=canonical{return Err(conflict("Request ID already used for different data"))}return serde_json::from_str(&result).map_err(|e|ApiError(StatusCode::INTERNAL_SERVER_ERROR,e.to_string()))}
  let task=get_item(&tx,id)?;
  if task["kind"]!="task"||!matches!(task["status"].as_str(),Some("inbox"|"ready"))||task["version"].as_i64()!=Some(input.expected_task_version){return Err(conflict("Task changed or cannot start a focus session"))}
  let active:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM app_focus_sessions WHERE state='active')",[],|r|r.get(0))?;
  if active{return Err(conflict("Finish the active focus session before starting another"))}
  let now=chrono::Utc::now().timestamp();
  tx.execute("INSERT INTO app_focus_sessions(reminder_id,task_id,started_at,planned_seconds,state) VALUES(NULL,?1,?2,?3,'active')",params![id,now,planned])?;
  let result=json!({"id":tx.last_insert_rowid(),"reminder_id":null,"task_id":id,"started_at":now,"planned_seconds":planned,"state":"active","version":0});
  tx.execute("INSERT INTO app_commands(device_id,request_id,input,result) VALUES(?1,?2,?3,?4)",params![input.device_id,input.request_id,canonical,result.to_string()])?;
  tx.commit()?;Ok(result)
 }).await?;s.revision.send_modify(|v|*v+=1);Ok(out)
}
pub async fn active_focus(State(s):State<AppState>)->ApiResult<Vec<Value>>{db_job(s,|c|{
 let mut q=c.prepare("SELECT f.id,f.reminder_id,f.task_id,f.started_at,f.planned_seconds,f.version,i.content FROM app_focus_sessions f JOIN app_items i ON i.id=f.task_id WHERE f.state='active' ORDER BY f.started_at")?;
 let rows=q.query_map([],|r|Ok(json!({"id":r.get::<_,i64>(0)?,"reminder_id":r.get::<_,Option<i64>>(1)?,"task_id":r.get::<_,i64>(2)?,"started_at":r.get::<_,i64>(3)?,"planned_seconds":r.get::<_,i64>(4)?,"version":r.get::<_,i64>(5)?,"title":r.get::<_,String>(6)?})))?.collect::<Result<Vec<_>,_>>()?;Ok(rows)
}).await}
#[derive(Deserialize,Serialize)] pub struct FinishFocus{device_id:String,request_id:String,expected_version:i64}
pub async fn finish_focus(State(s):State<AppState>,Path(id):Path<i64>,Json(input):Json<FinishFocus>)->ApiResult<Value>{
 if !valid_identity(&input.device_id,&input.request_id)||input.expected_version<0{return Err(invalid("Invalid focus session"))}
 let out=db_job(s.clone(),move|c|{
  let canonical=json!({"action":"finish_focus","session_id":id,"data":input}).to_string();let tx=c.transaction()?;
  let prior:Option<(String,String)>=tx.query_row("SELECT input,result FROM app_commands WHERE device_id=?1 AND request_id=?2",params![input.device_id,input.request_id],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
  if let Some((old,result))=prior{if old!=canonical{return Err(conflict("Request ID already used for different data"))}return serde_json::from_str(&result).map_err(|e|ApiError(StatusCode::INTERNAL_SERVER_ERROR,e.to_string()))}
  let row:Option<(i64,i64,i64,String)>=tx.query_row("SELECT started_at,planned_seconds,version,state FROM app_focus_sessions WHERE id=?1",[id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional()?;
  let(started,planned,version,state)=row.ok_or_else(||ApiError(StatusCode::NOT_FOUND,"Focus session not found".into()))?;
  if state!="active"||version!=input.expected_version{return Err(conflict("Focus session already ended or changed"))}
  let now=chrono::Utc::now().timestamp();let actual=(now-started).clamp(1,planned);let next=if actual>=planned{"finished"}else{"interrupted"};
  tx.execute("UPDATE app_focus_sessions SET ended_at=?1,actual_seconds=?2,state=?3,version=version+1 WHERE id=?4 AND version=?5",params![now,actual,next,id,input.expected_version])?;
  let result=json!({"id":id,"ended_at":now,"actual_seconds":actual,"state":next,"version":input.expected_version+1});
  tx.execute("INSERT INTO app_commands(device_id,request_id,input,result) VALUES(?1,?2,?3,?4)",params![input.device_id,input.request_id,canonical,result.to_string()])?;tx.commit()?;Ok(result)
 }).await?;s.revision.send_modify(|v|*v+=1);Ok(out)
}
#[derive(Deserialize,Serialize)] pub struct Restructure{device_id:String,request_id:String,expected_version:i64,content:String,reason:String,duration:i64,energy:Option<String>,due_at:i64}
pub async fn restructure(State(s):State<AppState>,Path(id):Path<i64>,Json(input):Json<Restructure>)->ApiResult<Value>{
 if !valid_identity(&input.device_id,&input.request_id)||input.expected_version<0||input.content.trim().is_empty()||input.content.chars().count()>20000||input.reason.trim().is_empty()||input.reason.chars().count()>2000||!(1..=1440).contains(&input.duration)||input.energy.as_deref().is_some_and(|v|!matches!(v,"low"|"medium"|"high"))||input.due_at<946684800||input.due_at>4102444800{return Err(invalid("Invalid replacement task"))}
 let out=db_job(s.clone(),move|c|{
  let canonical=json!({"action":"restructure","task_id":id,"data":input}).to_string();let tx=c.transaction()?;
  let prior:Option<(String,String)>=tx.query_row("SELECT input,result FROM app_commands WHERE device_id=?1 AND request_id=?2",params![input.device_id,input.request_id],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
  if let Some((old,result))=prior{if old!=canonical{return Err(conflict("Request ID already used for different data"))}return serde_json::from_str(&result).map_err(|e|ApiError(StatusCode::INTERNAL_SERVER_ERROR,e.to_string()))}
  let reminder=get(&tx,id)?;if reminder["state"]!="decision"{return Err(conflict("Task is not awaiting a decision or changed on another device"))}
  let task_id=reminder["task_id"].as_i64().ok_or_else(||invalid("Invalid task"))?;let before=get_item(&tx,task_id)?;
  if before["kind"]!="task"||before["status"]!="decision"||before["version"].as_i64()!=Some(input.expected_version){return Err(conflict("Task is not awaiting a decision or changed on another device"))}
  let original=before["content"].as_str().unwrap_or("");if original.trim().eq_ignore_ascii_case(input.content.trim()){return Err(invalid("The replacement must define a different concrete action"))}
  let now=chrono::Utc::now().timestamp();if input.due_at<=now{return Err(invalid("The replacement appointment must be in the future"))}
  let project_id=before["project_id"].as_i64();if let Some(pid)=project_id{let active:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM app_projects WHERE id=?1 AND status='active')",[pid],|r|r.get(0))?;if !active{return Err(conflict("Reopen or move the project before restructuring its task"))}}
  tx.execute("INSERT INTO app_items(kind,content,original_content,project_id,status,duration,energy) VALUES('task',?1,?1,?2,'ready',?3,?4)",params![input.content.trim(),project_id,input.duration,input.energy])?;let child=tx.last_insert_rowid();
  tx.execute("INSERT INTO app_task_relations(parent_task_id,child_task_id,relation,reason,created_at) VALUES(?1,?2,'restructured',?3,?4)",params![task_id,child,input.reason.trim(),now])?;
  tx.execute("INSERT INTO app_task_reminders(task_id,due_at,next_fire) VALUES(?1,?2,?2)",params![child,input.due_at])?;let child_reminder=tx.last_insert_rowid();
  tx.execute("INSERT INTO app_reminder_events(reminder_id,kind,at,device_id,due_at,reason) VALUES(?1,'schedule',?2,?3,?4,?5)",params![child_reminder,now,input.device_id,input.due_at,input.reason.trim()])?;
  tx.execute("UPDATE app_task_reminders SET closed_from=state,state='closed',version=version+1 WHERE task_id=?1 AND state='decision'",[task_id])?;
  tx.execute("UPDATE app_items SET status='restructured',version=version+1,updated_at=CURRENT_TIMESTAMP WHERE id=?1 AND version=?2",params![task_id,input.expected_version])?;
  let after=get_item(&tx,task_id)?;tx.execute("INSERT INTO app_item_history(item_id,action,before_state,after_state) VALUES(?1,'restructure',?2,?3)",params![task_id,before.to_string(),json!({"item":after,"replacement_task_id":child,"reason":input.reason.trim()}).to_string()])?;
  let result=json!({"original":after,"replacement":get_item(&tx,child)?,"reminder":get(&tx,child_reminder)?});
  tx.execute("INSERT INTO app_commands(device_id,request_id,input,result) VALUES(?1,?2,?3,?4)",params![input.device_id,input.request_id,canonical,result.to_string()])?;tx.commit()?;Ok(result)
 }).await?;s.revision.send_modify(|v|*v+=1);Ok(out)
}
#[derive(Deserialize,Serialize)] pub struct Reschedule{device_id:String,request_id:String,expected_version:i64,due_at:i64,reason:Option<String>}
pub async fn reschedule(State(s):State<AppState>,Path(id):Path<i64>,Json(input):Json<Reschedule>)->ApiResult<Value>{
 if !valid_identity(&input.device_id,&input.request_id)||input.expected_version<0||input.due_at<946684800||input.due_at>4102444800||input.reason.as_deref().unwrap_or("").chars().count()>2000{return Err(invalid("Invalid reschedule"))}
 let out=db_job(s.clone(),move|c|{
  let canonical=json!({"action":"reschedule_reminder","reminder_id":id,"data":input}).to_string();let tx=c.transaction()?;
  let prior:Option<(String,String)>=tx.query_row("SELECT input,result FROM app_commands WHERE device_id=?1 AND request_id=?2",params![input.device_id,input.request_id],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
  if let Some((old,result))=prior{if old!=canonical{return Err(conflict("Request ID already used for different data"))}return serde_json::from_str(&result).map_err(|e|ApiError(StatusCode::INTERNAL_SERVER_ERROR,e.to_string()))}
  let now=chrono::Utc::now().timestamp();
  if input.due_at<=now{return Err(invalid("The new appointment must be in the future"))}
  let before=get(&tx,id)?;
  if before["state"]!="scheduled"||before["version"].as_i64()!=Some(input.expected_version){return Err(conflict("Reminder changed or requires a decision"))}
  ensure_no_calendar_conflict(&tx,before["task_id"].as_i64().unwrap_or(0),input.due_at,Some(id))?;
  let overdue=before["due_at"].as_i64().unwrap_or(i64::MAX)<=now;
  let old=before["snoozes"].as_i64().unwrap_or(0);
  if overdue&&old>=3{return Err(conflict("The third appointment is due; a decision is required"))}
  if overdue&&input.reason.as_deref().unwrap_or("").trim().is_empty(){return Err(invalid("A reason is required when postponing"))}
  let count=old+i64::from(overdue);
  tx.execute("UPDATE app_task_reminders SET due_at=?1,next_fire=?1,snoozes=?2,reminder_index=0,last_reset_alert=COALESCE((SELECT MAX(id) FROM app_reminder_alerts WHERE reminder_id=?3),0),version=version+1 WHERE id=?3 AND version=?4",params![input.due_at,count,id,input.expected_version])?;
  tx.execute("INSERT INTO app_reminder_events(reminder_id,kind,at,device_id,reason,due_at) VALUES(?1,?2,?3,?4,?5,?6)",params![id,if overdue{"postpone"}else{"reschedule"},now,input.device_id,input.reason.as_deref().map(str::trim),input.due_at])?;
  let result=get(&tx,id)?;tx.execute("INSERT INTO app_commands(device_id,request_id,input,result) VALUES(?1,?2,?3,?4)",params![input.device_id,input.request_id,canonical,result.to_string()])?;
  tx.commit()?;Ok(result)
 }).await?;
 s.revision.send_modify(|v|*v+=1);Ok(out)
}
/// Coalesce missed intervals to one durable alert and schedule the next from now.
pub fn advance(c:&mut Connection,now:i64)->Result<usize,ApiError>{
 let tx=c.transaction()?;
 let due={let mut q=tx.prepare("SELECT id,snoozes,reminder_index FROM app_task_reminders WHERE state='scheduled' AND next_fire<=?1 ORDER BY next_fire,id")?;let rows=q.query_map([now],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,i64>(1)?,r.get::<_,i64>(2)?)))?.collect::<Result<Vec<_>,_>>()?;rows};
 for &(id,snoozes,index) in &due {
  if snoozes>=3 {
   tx.execute("UPDATE app_task_reminders SET state='decision',next_fire=NULL,version=version+1 WHERE id=?1",[id])?;
   tx.execute("UPDATE app_items SET status='decision',version=version+1,updated_at=CURRENT_TIMESTAMP WHERE id=(SELECT task_id FROM app_task_reminders WHERE id=?1)",[id])?;
   tx.execute("INSERT INTO app_reminder_events(reminder_id,kind,at) VALUES(?1,'decision',?2)",params![id,now])?;
   tx.execute("INSERT INTO app_reminder_alerts(reminder_id,created_at,kind) VALUES(?1,?2,'decision')",params![id,now])?;
  }else{
   tx.execute("INSERT INTO app_reminder_alerts(reminder_id,created_at) VALUES(?1,?2)",params![id,now])?;
   let delay=match index{0=>900,1=>3600,_=>10800};
   tx.execute("UPDATE app_task_reminders SET next_fire=?1,reminder_index=reminder_index+1,version=version+1 WHERE id=?2",params![now+delay,id])?;
  }
 }
 tx.commit()?;Ok(due.len())
}
pub async fn worker(s:AppState){loop{
 tokio::time::sleep(std::time::Duration::from_secs(1)).await;
 let changed={match s.db.lock(){Ok(mut c)=>{let now=chrono::Utc::now().timestamp();let finished=c.execute("UPDATE app_focus_sessions SET state='finished',ended_at=started_at+planned_seconds,actual_seconds=planned_seconds,version=version+1 WHERE state='active' AND started_at+planned_seconds<=?1",[now]).unwrap_or(0);match advance(&mut c,now){Ok(n)=>n+finished,Err(e)=>{eprintln!("Reminder scheduler failed: {}",e.1);finished}}},Err(_)=>0}};
 if changed>0{s.revision.send_modify(|v|*v+=1);}
}}

#[cfg(test)]
mod tests {
 use super::*;
 fn fixture()->Connection {
  let c=Connection::open_in_memory().unwrap();super::super::product::schema(&c).unwrap();schema(&c).unwrap();
  c.execute("INSERT INTO app_items(id,kind,content,original_content,status) VALUES(1,'task','task','task','ready')",[]).unwrap();
  c.execute("INSERT INTO app_task_reminders(id,task_id,due_at,next_fire) VALUES(1,1,100,100)",[]).unwrap();c
 }
 fn state(c:&Connection)->(String,Option<i64>,i64,i64){c.query_row("SELECT state,next_fire,snoozes,reminder_index FROM app_task_reminders WHERE id=1",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).unwrap()}
 #[test]
 fn missed_notifications_coalesce_and_follow_repeat_intervals(){
  let mut c=fixture();assert_eq!(advance(&mut c,100).unwrap(),1);assert_eq!(state(&c),("scheduled".into(),Some(1000),0,1));
  assert_eq!(advance(&mut c,999).unwrap(),0);assert_eq!(advance(&mut c,1000).unwrap(),1);assert_eq!(state(&c).1,Some(4600));
  // Simulate a server outage that skipped several intervals; only one alert appears.
  assert_eq!(advance(&mut c,20000).unwrap(),1);assert_eq!(state(&c).1,Some(30800));
  assert_eq!(c.query_row("SELECT count(*) FROM app_reminder_alerts",[],|r|r.get::<_,i64>(0)).unwrap(),3);
 }
 #[test]
 fn third_snooze_remains_active_until_due_then_requires_decision(){
  let mut c=fixture();c.execute("UPDATE app_task_reminders SET snoozes=3,due_at=500,next_fire=500",[]).unwrap();
  assert_eq!(advance(&mut c,499).unwrap(),0);assert_eq!(state(&c).0,"scheduled");
  assert_eq!(advance(&mut c,500).unwrap(),1);assert_eq!(state(&c),("decision".into(),None,3,0));
  assert_eq!(c.query_row("SELECT status FROM app_items WHERE id=1",[],|r|r.get::<_,String>(0)).unwrap(),"decision");
  assert_eq!(advance(&mut c,20000).unwrap(),0);assert_eq!(c.query_row("SELECT count(*) FROM app_reminder_alerts",[],|r|r.get::<_,i64>(0)).unwrap(),0);
 }
}
