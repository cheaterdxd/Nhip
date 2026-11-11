//! Proposed, reviewable multi-session plans for task scheduling.
use super::*;
use chrono::Timelike;

const LOCAL_OFFSET_SECONDS:i64=7*60*60;
const BREAK_BUFFER_SECONDS:i64=10*60;

#[derive(Deserialize)]
pub struct SlotQuery {task_id:i64,start:Option<String>,days:Option<i64>}

#[derive(Deserialize,Serialize,Clone)]
pub struct PlanSession {starts_at:i64,duration_minutes:i64}
#[derive(Deserialize,Serialize)]
pub struct ApplyPlan {device_id:String,request_id:String,task_id:i64,expected_task_version:i64,sessions:Vec<PlanSession>}
#[derive(Deserialize,Serialize)]
pub struct CancelPlan {device_id:String,request_id:String,expected_task_version:i64}
#[derive(Deserialize,Serialize)]
pub struct CompleteSession {device_id:String,request_id:String,expected_version:i64}

pub fn schema(c:&Connection)->rusqlite::Result<()>{c.execute_batch("CREATE TABLE IF NOT EXISTS app_schedule_plans(
 id INTEGER PRIMARY KEY,task_id INTEGER NOT NULL UNIQUE REFERENCES app_items(id),created_at INTEGER NOT NULL,state TEXT NOT NULL DEFAULT 'active' CHECK(state IN ('active','cancelled')));
 CREATE TABLE IF NOT EXISTS app_schedule_sessions(
 id INTEGER PRIMARY KEY,plan_id INTEGER NOT NULL REFERENCES app_schedule_plans(id),starts_at INTEGER NOT NULL,duration_minutes INTEGER NOT NULL CHECK(duration_minutes BETWEEN 1 AND 50),state TEXT NOT NULL DEFAULT 'planned' CHECK(state IN ('planned','done','cancelled')),version INTEGER NOT NULL DEFAULT 0,completed_at INTEGER);
 CREATE INDEX IF NOT EXISTS app_schedule_sessions_time ON app_schedule_sessions(starts_at,state);")?;
 let columns={let mut stmt=c.prepare("PRAGMA table_info(app_schedule_sessions)")?;let values=stmt.query_map([],|r|r.get::<_,String>(1))?.collect::<Result<Vec<_>,_>>()?;values};
 if !columns.iter().any(|column|column=="version"){c.execute_batch("ALTER TABLE app_schedule_sessions ADD COLUMN version INTEGER NOT NULL DEFAULT 0;")?;}
 if !columns.iter().any(|column|column=="completed_at"){c.execute_batch("ALTER TABLE app_schedule_sessions ADD COLUMN completed_at INTEGER;")?;}
 Ok(())}

fn daily_usage(c:&Connection,from:i64,to:i64,area:&str,exclude_task:i64)->Result<std::collections::HashMap<NaiveDate,i64>,ApiError>{
 let mut stmt=c.prepare("SELECT scheduled_at,minutes FROM (
  SELECT s.starts_at AS scheduled_at,s.duration_minutes AS minutes,coalesce(pr.area,'personal') AS area,i.id AS task_id FROM app_schedule_sessions s JOIN app_schedule_plans sp ON sp.id=s.plan_id JOIN app_items i ON i.id=sp.task_id LEFT JOIN app_projects pr ON pr.id=i.project_id WHERE (s.state='done' OR (sp.state='active' AND s.state='planned')) AND i.status NOT IN ('abandoned','restructured')
  UNION ALL
  SELECT r.due_at,coalesce(i.duration,25),coalesce(pr.area,'personal'),i.id FROM app_task_reminders r JOIN app_items i ON i.id=r.task_id LEFT JOIN app_projects pr ON pr.id=i.project_id WHERE r.state IN ('scheduled','decision') AND i.status NOT IN ('done','abandoned','restructured') AND NOT EXISTS(SELECT 1 FROM app_schedule_plans sp WHERE sp.task_id=i.id AND sp.state='active')
 ) WHERE scheduled_at>=?1 AND scheduled_at<?2 AND area=?3 AND task_id<>?4")?;
 let rows=stmt.query_map(params![from,to,area,exclude_task],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,i64>(1)?)))?.collect::<Result<Vec<_>,_>>()?;
 let mut usage=std::collections::HashMap::new();
 for (at,minutes) in rows{let local=chrono::DateTime::<chrono::Utc>::from_timestamp(at,0).ok_or_else(||ApiError(StatusCode::INTERNAL_SERVER_ERROR,"Invalid stored appointment time".into()))?.naive_utc()+Duration::seconds(LOCAL_OFFSET_SECONDS);*usage.entry(local.date()).or_insert(0)+=minutes;}
 Ok(usage)
}

fn get_plan(c:&Connection,plan_id:i64)->Result<Value,ApiError>{
 let (task_id,state,created_at):(i64,String,i64)=c.query_row("SELECT task_id,state,created_at FROM app_schedule_plans WHERE id=?1",[plan_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional()?.ok_or_else(||ApiError(StatusCode::NOT_FOUND,"Schedule plan not found".into()))?;
 let mut stmt=c.prepare("SELECT id,starts_at,duration_minutes,state,version,completed_at FROM app_schedule_sessions WHERE plan_id=?1 AND state IN ('planned','done') ORDER BY starts_at")?;
 let sessions=stmt.query_map([plan_id],|r|Ok(json!({"id":r.get::<_,i64>(0)?,"starts_at":r.get::<_,i64>(1)?,"duration_minutes":r.get::<_,i64>(2)?,"state":r.get::<_,String>(3)?,"version":r.get::<_,i64>(4)?,"completed_at":r.get::<_,Option<i64>>(5)?})))?.collect::<Result<Vec<_>,_>>()?;
 Ok(json!({"id":plan_id,"task_id":task_id,"state":state,"created_at":created_at,"sessions":sessions}))
}

pub async fn list(State(state):State<AppState>)->ApiResult<Vec<Value>>{db_job(state,|c|{
 let mut stmt=c.prepare("SELECT p.id FROM app_schedule_plans p JOIN app_items i ON i.id=p.task_id WHERE p.state='active' AND i.status NOT IN ('done','abandoned','restructured') ORDER BY p.created_at DESC")?;
 let ids=stmt.query_map([],|r|r.get::<_,i64>(0))?.collect::<Result<Vec<_>,_>>()?;
 ids.into_iter().map(|id|get_plan(c,id)).collect()
}).await}

fn energy_rank(energy:&str,minute:u32)->u32{
 match energy{
  "high"=>if (510..660).contains(&minute){0}else if (810..960).contains(&minute){1}else{2},
  "low"=>if (1080..1260).contains(&minute){0}else if minute>=960{1}else{2},
  _=>if (570..720).contains(&minute)||(840..1020).contains(&minute){0}else{1},
 }
}

pub async fn slots(State(state):State<AppState>,Query(query):Query<SlotQuery>)->ApiResult<Value>{
 let now=chrono::Utc::now().timestamp();
 let local_today=chrono::DateTime::<chrono::Utc>::from_timestamp(now,0).unwrap().naive_utc().checked_add_signed(Duration::seconds(LOCAL_OFFSET_SECONDS)).unwrap().date();
 let start=match query.start{
  Some(value)=>NaiveDate::parse_from_str(&value,"%Y-%m-%d").map_err(|_|ApiError(StatusCode::BAD_REQUEST,"start must be YYYY-MM-DD".into()))?,
  None=>local_today,
 };
 let days=query.days.unwrap_or(7);
 if start<local_today||!(1..=30).contains(&days){return Err(ApiError(StatusCode::BAD_REQUEST,"Choose today or a future date and 1–30 days".into()))}
 let from=start.and_hms_opt(0,0,0).unwrap().and_utc().timestamp()-LOCAL_OFFSET_SECONDS;
 let to=(start+Duration::days(days)).and_hms_opt(0,0,0).unwrap().and_utc().timestamp()-LOCAL_OFFSET_SECONDS;
 db_job(state,move|c|{
  let task:Option<(Option<i64>,Option<String>,String,String)>=c.query_row("SELECT i.duration,i.energy,coalesce(p.area,'personal'),i.status FROM app_items i LEFT JOIN app_projects p ON p.id=i.project_id WHERE i.id=?1 AND i.kind='task'",[query.task_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional()?;
  let(duration,energy,area,status)=task.ok_or_else(||ApiError(StatusCode::NOT_FOUND,"Task not found".into()))?;
  if !matches!(status.as_str(),"inbox"|"ready"){return Err(ApiError(StatusCode::CONFLICT,"Only open tasks can be scheduled".into()))}
  let total_duration=duration.unwrap_or(25);
  if !(1..=1440).contains(&total_duration){return Err(ApiError(StatusCode::BAD_REQUEST,"Task duration must be between 1 and 1440 minutes".into()))}
  let completed_minutes:i64=c.query_row("SELECT coalesce(sum(s.duration_minutes),0) FROM app_schedule_sessions s JOIN app_schedule_plans p ON p.id=s.plan_id WHERE p.task_id=?1 AND s.state='done'",[query.task_id],|r|r.get(0))?;
  let duration=total_duration-completed_minutes;if duration<=0{return Err(ApiError(StatusCode::CONFLICT,"All planned sessions are complete; finish or edit the task instead".into()))}
  let energy=energy.unwrap_or_else(||"medium".into());
  let mut busy=Vec::<(i64,i64)>::new();
  {
   let mut q=c.prepare("SELECT r.due_at,coalesce(i.duration,25) FROM app_task_reminders r JOIN app_items i ON i.id=r.task_id WHERE r.task_id<>?1 AND r.state IN ('scheduled','decision') AND r.due_at>=?2 AND r.due_at<?3")?;
   for row in q.query_map(params![query.task_id,from,to],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,i64>(1)?)))?{
    let(at,minutes)=row?;busy.push((at,at+minutes*60));
   }
  }
  {
   let mut q=c.prepare("SELECT started_at,started_at+planned_seconds FROM app_focus_sessions WHERE state='active'")?;
   for row in q.query_map([],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,i64>(1)?)))?{busy.push(row?);}
  }
  {
   let mut q=c.prepare("SELECT starts_at,ends_at FROM app_busy_events WHERE starts_at<?2 AND ends_at>?1")?;
   for row in q.query_map(params![from,to],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,i64>(1)?)))?{busy.push(row?);}
  }
  {
   let mut q=c.prepare("SELECT s.starts_at,s.starts_at+s.duration_minutes*60 FROM app_schedule_sessions s JOIN app_schedule_plans p ON p.id=s.plan_id WHERE p.task_id<>?1 AND p.state='active' AND s.state='planned' AND s.starts_at<?3 AND s.starts_at+s.duration_minutes*60>?2")?;
   for row in q.query_map(params![query.task_id,from,to],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,i64>(1)?)))?{busy.push(row?);}
  }
  let usage=daily_usage(c,from,to,&area,query.task_id)?;
  let mut sessions=Vec::<PlanSession>::new();let mut remaining=duration;let daily_capacity=if area=="company"{300}else{120};
  for offset in 0..days{
   if remaining<=0{break}
   let date=start+Duration::days(offset);
   if date.weekday().num_days_from_monday()>=5{continue}
   let windows:Vec<(u32,u32)>=if area=="company"{vec![(510,720),(810,1080)]}else{vec![(420,510),(720,810),(1080,1440)]};
   let mut used=usage.get(&date).copied().unwrap_or(0);
   while remaining>0&&used<daily_capacity{
    let block=remaining.min(50).min(daily_capacity-used);let mut candidates=Vec::<(u32,u32,i64)>::new();
    for (begin,end) in &windows{
     let mut minute=*begin;
     while minute+block as u32<=*end{
      let local=date.and_hms_opt(minute/60,minute%60,0).unwrap();let at=local.and_utc().timestamp()-LOCAL_OFFSET_SECONDS;let finish=at+block*60;
      if at>now+60&&!busy.iter().any(|(busy_start,busy_end)|at<*busy_end+BREAK_BUFFER_SECONDS&&*busy_start-BREAK_BUFFER_SECONDS<finish){candidates.push((energy_rank(&energy,minute),minute,at));}
      minute+=15;
     }
    }
    if let Some((_,_,at))=candidates.into_iter().min_by_key(|(rank,minute,_)|(*rank,*minute)){
     sessions.push(PlanSession{starts_at:at,duration_minutes:block});busy.push((at,at+block*60));remaining-=block;used+=block;
    }else{break}
   }
  }
  sessions.sort_by_key(|session|session.starts_at);
  let reason=if area=="company"{"Đã xét giờ công ty, mức tập trung, khoảng nghỉ và sức chứa tối đa 5 giờ mỗi ngày."}else{"Đã xét giờ cá nhân, mức tập trung, khoảng nghỉ và sức chứa tối đa 2 giờ mỗi ngày."};
  Ok(json!({"task_id":query.task_id,"duration_minutes":duration,"remaining_minutes":remaining,"complete":remaining==0,"energy":energy,"area":area,"start":start.to_string(),"days":days,"sessions":sessions,"reason":reason,"applied":false,"policy":{"weekdays_only":true,"max_session_minutes":50,"daily_capacity_minutes":daily_capacity,"personal_windows":[[420,510],[720,810],[1080,1440]],"company_windows":[[510,720],[810,1080]],"break_buffer_minutes":10,"step_minutes":15}}))
 }).await
}

pub async fn apply(State(state):State<AppState>,Json(input):Json<ApplyPlan>)->ApiResult<Value>{
 if input.device_id.is_empty()||input.device_id.len()>100||input.request_id.is_empty()||input.request_id.len()>100||input.expected_task_version<0||input.sessions.is_empty()||input.sessions.len()>30||input.sessions.iter().any(|s|!(1..=50).contains(&s.duration_minutes)||!(946684800..=4102444800).contains(&s.starts_at)){return Err(ApiError(StatusCode::BAD_REQUEST,"Invalid schedule plan".into()))}
 let out=db_job(state.clone(),move|c|{
  let canonical=serde_json::to_string(&input).map_err(|e|ApiError(StatusCode::BAD_REQUEST,e.to_string()))?;let tx=c.transaction()?;
  if let Some((old,result))=tx.query_row("SELECT input,result FROM app_commands WHERE device_id=?1 AND request_id=?2",params![input.device_id,input.request_id],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?))).optional()?{if old!=canonical{return Err(ApiError(StatusCode::CONFLICT,"Request ID already used for different data".into()))}return serde_json::from_str(&result).map_err(|e|ApiError(StatusCode::INTERNAL_SERVER_ERROR,e.to_string()))}
  let task:Option<(i64,String,String,i64)>=tx.query_row("SELECT coalesce(i.duration,25),coalesce(p.area,'personal'),i.status,i.version FROM app_items i LEFT JOIN app_projects p ON p.id=i.project_id WHERE i.id=?1 AND i.kind='task'",[input.task_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional()?;
  let(total,area,status,version)=task.ok_or_else(||ApiError(StatusCode::NOT_FOUND,"Task not found".into()))?;
  if !matches!(status.as_str(),"inbox"|"ready")||version!=input.expected_task_version{return Err(ApiError(StatusCode::CONFLICT,"Task changed or cannot be scheduled".into()))}
  if tx.query_row("SELECT EXISTS(SELECT 1 FROM app_task_reminders WHERE task_id=?1 AND state<>'closed')",[input.task_id],|r|r.get::<_,bool>(0))?||tx.query_row("SELECT EXISTS(SELECT 1 FROM app_schedule_plans WHERE task_id=?1 AND state='active')",[input.task_id],|r|r.get::<_,bool>(0))?{return Err(ApiError(StatusCode::CONFLICT,"Task already has a scheduled commitment".into()))}
  let completed_minutes:i64=tx.query_row("SELECT coalesce(sum(s.duration_minutes),0) FROM app_schedule_sessions s JOIN app_schedule_plans p ON p.id=s.plan_id WHERE p.task_id=?1 AND s.state='done'",[input.task_id],|r|r.get(0))?;
  let remaining=total-completed_minutes;if remaining<=0||input.sessions.iter().map(|s|s.duration_minutes).sum::<i64>()!=remaining{return Err(ApiError(StatusCode::BAD_REQUEST,"Planned session durations must equal the task’s remaining estimated time".into()))}
  let now=chrono::Utc::now().timestamp();let daily_capacity=if area=="company"{300}else{120};
  let first_day=chrono::DateTime::<chrono::Utc>::from_timestamp(input.sessions.first().unwrap().starts_at,0).unwrap().naive_utc().checked_add_signed(Duration::seconds(LOCAL_OFFSET_SECONDS)).unwrap().date();
  let last_day=chrono::DateTime::<chrono::Utc>::from_timestamp(input.sessions.last().unwrap().starts_at,0).unwrap().naive_utc().checked_add_signed(Duration::seconds(LOCAL_OFFSET_SECONDS)).unwrap().date();
  let from=first_day.and_hms_opt(0,0,0).unwrap().and_utc().timestamp()-LOCAL_OFFSET_SECONDS;let to=(last_day+Duration::days(1)).and_hms_opt(0,0,0).unwrap().and_utc().timestamp()-LOCAL_OFFSET_SECONDS;
  let mut day_totals=daily_usage(&tx,from,to,&area,input.task_id)?;let mut previous_end=0i64;
  for session in &input.sessions{
   if !(1..=50).contains(&session.duration_minutes)||session.starts_at<=now||session.starts_at<previous_end+BREAK_BUFFER_SECONDS{return Err(ApiError(StatusCode::BAD_REQUEST,"Sessions need future times, 1–50 minutes each, and a 10-minute break".into()))}
   let local=chrono::DateTime::<chrono::Utc>::from_timestamp(session.starts_at,0).ok_or_else(||ApiError(StatusCode::BAD_REQUEST,"Invalid session time".into()))?.naive_utc()+Duration::seconds(LOCAL_OFFSET_SECONDS);
   if local.weekday().num_days_from_monday()>=5{return Err(ApiError(StatusCode::CONFLICT,"Tasks are not scheduled on weekends".into()))}
   let minute=local.hour()*60+local.minute();let end=minute+session.duration_minutes as u32;let allowed=if area=="company"{(minute>=510&&end<=720)||(minute>=810&&end<=1080)}else{(minute>=420&&end<=510)||(minute>=720&&end<=810)||(minute>=1080&&end<=1440)};
   if !allowed{return Err(ApiError(StatusCode::CONFLICT,"Session is outside this task’s work windows".into()))}
   let day=local.date();let used=day_totals.entry(day).or_default();*used+=session.duration_minutes;if *used>daily_capacity{return Err(ApiError(StatusCode::CONFLICT,"Plan exceeds the daily task capacity".into()))}
   let overlaps:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM app_busy_events WHERE starts_at<?2 AND ends_at>?1)",params![session.starts_at-600,session.starts_at+session.duration_minutes*60+600],|r|r.get(0))?;
   let other:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM app_task_reminders r JOIN app_items i ON i.id=r.task_id WHERE r.state IN ('scheduled','decision') AND r.task_id<>?1 AND r.due_at<?3 AND r.due_at+coalesce(i.duration,25)*60>?2)",params![input.task_id,session.starts_at-600,session.starts_at+session.duration_minutes*60+600],|r|r.get(0))?;
   let planned:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM app_schedule_sessions s JOIN app_schedule_plans p ON p.id=s.plan_id WHERE p.task_id<>?1 AND p.state='active' AND s.state='planned' AND s.starts_at<?3 AND s.starts_at+s.duration_minutes*60>?2)",params![input.task_id,session.starts_at-600,session.starts_at+session.duration_minutes*60+600],|r|r.get(0))?;
   let focus:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM app_focus_sessions WHERE state='active' AND started_at<?2 AND started_at+planned_seconds>?1)",params![session.starts_at-600,session.starts_at+session.duration_minutes*60+600],|r|r.get(0))?;
   if overlaps||other||planned||focus{return Err(ApiError(StatusCode::CONFLICT,"A suggested session now conflicts with another calendar commitment; find times again".into()))}
   previous_end=session.starts_at+session.duration_minutes*60;
  }
  let existing_plan:Option<i64>=tx.query_row("SELECT id FROM app_schedule_plans WHERE task_id=?1",[input.task_id],|r|r.get(0)).optional()?;
  let plan_id=if let Some(id)=existing_plan{tx.execute("UPDATE app_schedule_plans SET state='active',created_at=?1 WHERE id=?2",params![now,id])?;tx.execute("UPDATE app_schedule_sessions SET state='cancelled' WHERE plan_id=?1 AND state='planned'",[id])?;id}else{tx.execute("INSERT INTO app_schedule_plans(task_id,created_at) VALUES(?1,?2)",params![input.task_id,now])?;tx.last_insert_rowid()};
  for session in &input.sessions{tx.execute("INSERT INTO app_schedule_sessions(plan_id,starts_at,duration_minutes) VALUES(?1,?2,?3)",params![plan_id,session.starts_at,session.duration_minutes])?;}
  let first=input.sessions[0].starts_at;
  let existing_reminder:Option<(i64,String)>=tx.query_row("SELECT id,state FROM app_task_reminders WHERE task_id=?1",[input.task_id],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
  let reminder_id=if let Some((id,_))=existing_reminder{tx.execute("UPDATE app_task_reminders SET due_at=?1,next_fire=?1,snoozes=0,reminder_index=0,state='scheduled',closed_from=NULL,version=version+1 WHERE id=?2",params![first,id])?;id}else{tx.execute("INSERT INTO app_task_reminders(task_id,due_at,next_fire) VALUES(?1,?2,?2)",params![input.task_id,first])?;tx.last_insert_rowid()};
  tx.execute("UPDATE app_items SET version=version+1,updated_at=CURRENT_TIMESTAMP WHERE id=?1 AND version=?2",params![input.task_id,input.expected_task_version])?;
  let result=json!({"plan_id":plan_id,"task_id":input.task_id,"sessions":input.sessions,"first_reminder_at":first,"applied":true});
  tx.execute("INSERT INTO app_reminder_events(reminder_id,kind,at,device_id,due_at) VALUES(?1,'schedule_plan',?2,?3,?4)",params![reminder_id,now,input.device_id,first])?;
  tx.execute("INSERT INTO app_commands(device_id,request_id,input,result) VALUES(?1,?2,?3,?4)",params![input.device_id,input.request_id,canonical,result.to_string()])?;tx.commit()?;Ok(result)
 }).await?;state.revision.send_modify(|v|*v+=1);Ok(out)
}

pub async fn cancel(State(state):State<AppState>,Path(plan_id):Path<i64>,Json(input):Json<CancelPlan>)->ApiResult<Value>{
 if input.device_id.is_empty()||input.device_id.len()>100||input.request_id.is_empty()||input.request_id.len()>100||input.expected_task_version<0{return Err(ApiError(StatusCode::BAD_REQUEST,"Invalid plan cancellation".into()))}
 let out=db_job(state.clone(),move|c|{
  let canonical=json!({"action":"cancel_schedule_plan","plan_id":plan_id,"data":input}).to_string();let tx=c.transaction()?;
  if let Some((old,result))=tx.query_row("SELECT input,result FROM app_commands WHERE device_id=?1 AND request_id=?2",params![input.device_id,input.request_id],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?))).optional()?{if old!=canonical{return Err(ApiError(StatusCode::CONFLICT,"Request ID already used for different data".into()))}return serde_json::from_str(&result).map_err(|e|ApiError(StatusCode::INTERNAL_SERVER_ERROR,e.to_string()))}
  let (task_id,state):(i64,String)=tx.query_row("SELECT task_id,state FROM app_schedule_plans WHERE id=?1",[plan_id],|r|Ok((r.get(0)?,r.get(1)?))).optional()?.ok_or_else(||ApiError(StatusCode::NOT_FOUND,"Schedule plan not found".into()))?;
  if state!="active"{return Err(ApiError(StatusCode::CONFLICT,"Schedule plan is already cancelled".into()))}
  let version:i64=tx.query_row("SELECT version FROM app_items WHERE id=?1",[task_id],|r|r.get(0))?;
  if version!=input.expected_task_version{return Err(ApiError(StatusCode::CONFLICT,"Task changed; refresh before cancelling".into()))}
  let active_focus:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM app_focus_sessions WHERE task_id=?1 AND state='active')",[task_id],|r|r.get(0))?;
  if active_focus{return Err(ApiError(StatusCode::CONFLICT,"End the active focus session before cancelling its plan".into()))}
  let reminder:Option<(i64,String)>=tx.query_row("SELECT id,state FROM app_task_reminders WHERE task_id=?1",[task_id],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
  if let Some((id,reminder_state))=reminder{if reminder_state!="closed"{tx.execute("UPDATE app_task_reminders SET closed_from=state,state='closed',next_fire=NULL,version=version+1 WHERE id=?1",[id])?;tx.execute("INSERT INTO app_reminder_events(reminder_id,kind,at,device_id) VALUES(?1,'cancel_plan',?2,?3)",params![id,chrono::Utc::now().timestamp(),input.device_id])?;}}
  tx.execute("UPDATE app_schedule_plans SET state='cancelled' WHERE id=?1",[plan_id])?;
  tx.execute("UPDATE app_schedule_sessions SET state='cancelled' WHERE plan_id=?1 AND state='planned'",[plan_id])?;
  tx.execute("UPDATE app_items SET version=version+1,updated_at=CURRENT_TIMESTAMP WHERE id=?1 AND version=?2",params![task_id,input.expected_task_version])?;
  let result=json!({"plan_id":plan_id,"task_id":task_id,"state":"cancelled"});
  tx.execute("INSERT INTO app_item_history(item_id,action,before_state,after_state) VALUES(?1,'cancel_schedule_plan',?2,?3)",params![task_id,json!({"version":version,"plan_id":plan_id}).to_string(),result.to_string()])?;
  tx.execute("INSERT INTO app_commands(device_id,request_id,input,result) VALUES(?1,?2,?3,?4)",params![input.device_id,input.request_id,canonical,result.to_string()])?;tx.commit()?;Ok(result)
 }).await?;state.revision.send_modify(|v|*v+=1);Ok(out)
}

pub async fn complete_session(State(state):State<AppState>,Path((plan_id,session_id)):Path<(i64,i64)>,Json(input):Json<CompleteSession>)->ApiResult<Value>{
 if input.device_id.is_empty()||input.device_id.len()>100||input.request_id.is_empty()||input.request_id.len()>100||input.expected_version<0{return Err(ApiError(StatusCode::BAD_REQUEST,"Invalid session completion".into()))}
 let out=db_job(state.clone(),move|c|{
  let canonical=json!({"action":"complete_schedule_session","plan_id":plan_id,"session_id":session_id,"data":input}).to_string();let tx=c.transaction()?;
  if let Some((old,result))=tx.query_row("SELECT input,result FROM app_commands WHERE device_id=?1 AND request_id=?2",params![input.device_id,input.request_id],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?))).optional()?{if old!=canonical{return Err(ApiError(StatusCode::CONFLICT,"Request ID already used for different data".into()))}return serde_json::from_str(&result).map_err(|e|ApiError(StatusCode::INTERNAL_SERVER_ERROR,e.to_string()))}
  let (task_id,starts_at,duration,state,version):(i64,i64,i64,String,i64)=tx.query_row("SELECT p.task_id,s.starts_at,s.duration_minutes,s.state,s.version FROM app_schedule_sessions s JOIN app_schedule_plans p ON p.id=s.plan_id WHERE s.id=?1 AND p.id=?2",params![session_id,plan_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?))).optional()?.ok_or_else(||ApiError(StatusCode::NOT_FOUND,"Scheduled session not found".into()))?;
  let active:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM app_schedule_plans WHERE id=?1 AND state='active')",[plan_id],|r|r.get(0))?;
  if !active||state!="planned"||version!=input.expected_version{return Err(ApiError(StatusCode::CONFLICT,"Session changed or is no longer planned".into()))}
  let earlier:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM app_schedule_sessions WHERE plan_id=?1 AND state='planned' AND starts_at<?2)",params![plan_id,starts_at],|r|r.get(0))?;
  if earlier{return Err(ApiError(StatusCode::CONFLICT,"Complete planned sessions in order".into()))}
  let now=chrono::Utc::now().timestamp();tx.execute("UPDATE app_schedule_sessions SET state='done',completed_at=?1,version=version+1 WHERE id=?2 AND version=?3",params![now,session_id,input.expected_version])?;
  let next:Option<i64>=tx.query_row("SELECT starts_at FROM app_schedule_sessions WHERE plan_id=?1 AND state='planned' ORDER BY starts_at LIMIT 1",[plan_id],|r|r.get(0)).optional()?;
  let reminder:Option<i64>=tx.query_row("SELECT id FROM app_task_reminders WHERE task_id=?1",[task_id],|r|r.get(0)).optional()?;
  if let (Some(reminder_id),Some(next_at))=(reminder,next){tx.execute("UPDATE app_task_reminders SET due_at=?1,next_fire=?1,reminder_index=0,last_reset_alert=COALESCE((SELECT MAX(id) FROM app_reminder_alerts WHERE reminder_id=?2),0),state='scheduled',closed_from=NULL,version=version+1 WHERE id=?2",params![next_at,reminder_id])?;tx.execute("INSERT INTO app_reminder_events(reminder_id,kind,at,device_id,due_at) VALUES(?1,'advance_schedule_session',?2,?3,?4)",params![reminder_id,now,input.device_id,next_at])?;}
  else if let Some(reminder_id)=reminder{tx.execute("UPDATE app_task_reminders SET closed_from=state,state='closed',next_fire=NULL,version=version+1 WHERE id=?1",[reminder_id])?;tx.execute("INSERT INTO app_reminder_events(reminder_id,kind,at,device_id) VALUES(?1,'finish_schedule_plan',?2,?3)",params![reminder_id,now,input.device_id])?;}
  let result=json!({"plan_id":plan_id,"session_id":session_id,"task_id":task_id,"starts_at":starts_at,"duration_minutes":duration,"state":"done","completed_at":now,"version":input.expected_version+1});
  tx.execute("INSERT INTO app_item_history(item_id,action,before_state,after_state) VALUES(?1,'schedule_session_complete',?2,?3)",params![task_id,json!({"session_id":session_id,"state":"planned"}).to_string(),result.to_string()])?;
  tx.execute("INSERT INTO app_commands(device_id,request_id,input,result) VALUES(?1,?2,?3,?4)",params![input.device_id,input.request_id,canonical,result.to_string()])?;tx.commit()?;Ok(result)
 }).await?;state.revision.send_modify(|v|*v+=1);Ok(out)
}
