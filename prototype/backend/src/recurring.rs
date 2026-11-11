//! Calendar-based task rules. Occurrences are materialized once with a unique
//! (rule, local date) key so missed dates survive restarts without duplicates.
use super::*;
use crate::product::get_item;

const LOCAL_OFFSET_SECONDS:i64=7*60*60; // Asia/Ho_Chi_Minh has no DST.

pub fn schema(c:&Connection)->rusqlite::Result<()>{c.execute_batch("CREATE TABLE IF NOT EXISTS app_recurring_rules(
 id INTEGER PRIMARY KEY,template_task_id INTEGER NOT NULL REFERENCES app_items(id),frequency TEXT NOT NULL CHECK(frequency IN ('daily','weekly','monthly')),
 weekdays TEXT NOT NULL DEFAULT '[]',month_day INTEGER,local_time TEXT NOT NULL,starts_on TEXT NOT NULL,status TEXT NOT NULL DEFAULT 'active' CHECK(status IN ('active','paused','ended')),
 version INTEGER NOT NULL DEFAULT 0,resume_after TEXT NOT NULL DEFAULT '1970-01-01',created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP);
 CREATE TABLE IF NOT EXISTS app_recurring_occurrences(id INTEGER PRIMARY KEY,rule_id INTEGER NOT NULL REFERENCES app_recurring_rules(id),occurrence_date TEXT NOT NULL,task_id INTEGER NOT NULL UNIQUE REFERENCES app_items(id),due_at INTEGER NOT NULL,UNIQUE(rule_id,occurrence_date));
 CREATE INDEX IF NOT EXISTS app_recurring_occurrence_rule_date ON app_recurring_occurrences(rule_id,occurrence_date);")?;
 let has_resume_after={let mut q=c.prepare("PRAGMA table_info(app_recurring_rules)")?;let cols=q.query_map([],|r|r.get::<_,String>(1))?.collect::<Result<Vec<_>,_>>()?;cols.iter().any(|x|x=="resume_after")};
 if !has_resume_after{c.execute_batch("ALTER TABLE app_recurring_rules ADD COLUMN resume_after TEXT NOT NULL DEFAULT '1970-01-01';")?;}Ok(())}

fn bad(s:&str)->ApiError{ApiError(StatusCode::BAD_REQUEST,s.into())}
fn conflict(s:&str)->ApiError{ApiError(StatusCode::CONFLICT,s.into())}
fn today_local(now:i64)->NaiveDate{chrono::DateTime::<chrono::Utc>::from_timestamp(now,0).unwrap_or_else(||chrono::Utc::now()).naive_utc().checked_add_signed(Duration::seconds(LOCAL_OFFSET_SECONDS)).unwrap().date()}
fn due_at(date:NaiveDate,time:&str)->Option<i64>{
 let mut parts=time.split(':');let hour=parts.next()?.parse::<u32>().ok()?;let minute=parts.next()?.parse::<u32>().ok()?;if parts.next().is_some(){return None}
 date.and_hms_opt(hour,minute,0).map(|dt|dt.and_utc().timestamp()-LOCAL_OFFSET_SECONDS)
}
fn scheduled_date(date:NaiveDate)->NaiveDate{match date.weekday().num_days_from_monday(){5=>date+Duration::days(2),6=>date+Duration::days(1),_=>date}}
fn matches_date(freq:&str,weekdays:&[u32],month_day:Option<u32>,date:NaiveDate)->bool{match freq{
 "daily"=>date.weekday().num_days_from_monday()<5,
 "weekly"=>weekdays.contains(&(date.weekday().num_days_from_monday())),
 "monthly"=>{let wanted=month_day.unwrap_or(1);let next=if date.month()==12{NaiveDate::from_ymd_opt(date.year()+1,1,1)}else{NaiveDate::from_ymd_opt(date.year(),date.month()+1,1)};let last=next.and_then(|d|d.pred_opt()).map(|d|d.day()).unwrap_or(31);date.day()==wanted.min(last)},
 _=>false}}

pub fn generate(c:&mut Connection,now:i64)->Result<usize,ApiError>{
 let tx=c.transaction()?;let c=&tx;
 let today=today_local(now);let horizon=today+Duration::days(31);let mut q=c.prepare("SELECT id,template_task_id,frequency,weekdays,month_day,local_time,starts_on,resume_after FROM app_recurring_rules WHERE status='active' ORDER BY id")?;
 let rules=q.query_map([],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,i64>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?,r.get::<_,Option<u32>>(4)?,r.get::<_,String>(5)?,r.get::<_,String>(6)?,r.get::<_,String>(7)?)))?.collect::<Result<Vec<_>,_>>()?;drop(q);
 let mut created=0;
 for(rule_id,template,frequency,days_json,month_day,time,starts,resume_after) in rules{
  let days:Vec<u32>=serde_json::from_str(&days_json).map_err(|e|ApiError(StatusCode::INTERNAL_SERVER_ERROR,e.to_string()))?;
  let start=NaiveDate::parse_from_str(&starts,"%Y-%m-%d").map_err(|e|ApiError(StatusCode::INTERNAL_SERVER_ERROR,e.to_string()))?;
  let resume=NaiveDate::parse_from_str(&resume_after,"%Y-%m-%d").unwrap_or(today);
  let last:Option<String>=c.query_row("SELECT MAX(occurrence_date) FROM app_recurring_occurrences WHERE rule_id=?1",[rule_id],|r|r.get(0))?;
  let mut date=match last{Some(value)=>{let last=NaiveDate::parse_from_str(&value,"%Y-%m-%d").map_err(|e|ApiError(StatusCode::INTERNAL_SERVER_ERROR,e.to_string()))?;if last>today{continue}(last+Duration::days(1)).max(resume)},None=>start.max(resume)};
  while date<=horizon{
   if matches_date(&frequency,&days,month_day,date){
    let at=due_at(scheduled_date(date),&time).ok_or_else(||ApiError(StatusCode::INTERNAL_SERVER_ERROR,"Invalid stored recurrence time".into()))?;
    if date==start{
     // The selected task is the first occurrence; only create its reminder.
     let exists:bool=c.query_row("SELECT EXISTS(SELECT 1 FROM app_recurring_occurrences WHERE rule_id=?1 AND occurrence_date=?2)",params![rule_id,date.to_string()],|r|r.get(0))?;
     if !exists{c.execute("INSERT INTO app_recurring_occurrences(rule_id,occurrence_date,task_id,due_at) VALUES(?1,?2,?3,?4)",params![rule_id,date.to_string(),template,at])?;c.execute("INSERT OR IGNORE INTO app_task_reminders(task_id,due_at,next_fire) VALUES(?1,?2,?2)",params![template,at])?;let rid=c.last_insert_rowid();c.execute("INSERT INTO app_reminder_events(reminder_id,kind,at,due_at) VALUES(?1,'schedule',?2,?3)",params![rid,now,at])?;created+=1;}
    }else{
     let source:Option<(String,Option<i64>,Option<i64>,Option<String>)>=c.query_row("SELECT content,project_id,duration,energy FROM app_items WHERE id=?1",[template],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional()?;
     if let Some((content,project,duration,energy))=source{
      c.execute("INSERT OR IGNORE INTO app_items(kind,content,original_content,project_id,status,duration,energy) VALUES('task',?1,?1,?2,'ready',?3,?4)",params![content,project,duration,energy])?;
 let task=c.last_insert_rowid();
      let inserted=c.execute("INSERT OR IGNORE INTO app_recurring_occurrences(rule_id,occurrence_date,task_id,due_at) VALUES(?1,?2,?3,?4)",params![rule_id,date.to_string(),task,at])?;
      if inserted==1{c.execute("INSERT INTO app_task_reminders(task_id,due_at,next_fire) VALUES(?1,?2,?2)",params![task,at])?;let rid=c.last_insert_rowid();c.execute("INSERT INTO app_reminder_events(reminder_id,kind,at,due_at) VALUES(?1,'schedule',?2,?3)",params![rid,now,at])?;created+=1}else{c.execute("DELETE FROM app_items WHERE id=?1",[task])?;}
     }
    }
    if date>today{break;}
   }
   date=date+Duration::days(1);
  }
 }
 tx.commit()?;Ok(created)
}

pub async fn list(State(s):State<AppState>)->ApiResult<Vec<Value>>{db_job(s,|c|{
 let mut q=c.prepare("SELECT r.id,r.template_task_id,i.content,r.frequency,r.weekdays,r.month_day,r.local_time,r.starts_on,r.status,r.version,(SELECT count(*) FROM app_recurring_occurrences o WHERE o.rule_id=r.id),(SELECT count(*) FROM app_recurring_occurrences o JOIN app_items x ON x.id=o.task_id WHERE o.rule_id=r.id AND x.status NOT IN ('done','abandoned','restructured')) FROM app_recurring_rules r JOIN app_items i ON i.id=r.template_task_id ORDER BY r.id DESC")?;
 let rows=q.query_map([],|r|Ok(json!({"id":r.get::<_,i64>(0)?,"template_task_id":r.get::<_,i64>(1)?,"content":r.get::<_,String>(2)?,"frequency":r.get::<_,String>(3)?,"weekdays":serde_json::from_str::<Vec<u32>>(&r.get::<_,String>(4)?).unwrap_or_default(),"month_day":r.get::<_,Option<u32>>(5)?,"local_time":r.get::<_,String>(6)?,"starts_on":r.get::<_,String>(7)?,"status":r.get::<_,String>(8)?,"version":r.get::<_,i64>(9)?,"occurrence_count":r.get::<_,i64>(10)?,"open_count":r.get::<_,i64>(11)?})))?.collect::<Result<Vec<_>,_>>()?;Ok(rows)
}).await}

#[derive(Deserialize,Serialize)]pub struct CreateRule{device_id:String,request_id:String,expected_task_version:i64,task_id:i64,frequency:String,weekdays:Vec<u32>,month_day:Option<u32>,local_time:String,starts_on:String}
pub async fn create(State(s):State<AppState>,Json(input):Json<CreateRule>)->ApiResult<Value>{
 let today=today_local(chrono::Utc::now().timestamp());let start=NaiveDate::parse_from_str(&input.starts_on,"%Y-%m-%d").ok();
 if input.device_id.is_empty()||input.device_id.len()>100||input.request_id.is_empty()||input.request_id.len()>100||input.expected_task_version<0||!matches!(input.frequency.as_str(),"daily"|"weekly"|"monthly")||start.is_none_or(|d|d<today)||due_at(start.unwrap_or(today),&input.local_time).is_none(){return Err(bad("Invalid recurrence rule"))}
 let start=start.unwrap();let mut days=input.weekdays.clone();days.sort_unstable();days.dedup();
 let rule_valid=match input.frequency.as_str(){"daily"=>days.is_empty()&&input.month_day.is_none()&&matches_date("daily",&[],None,start),"weekly"=>!days.is_empty()&&days.iter().all(|d|*d<5)&&days.contains(&start.weekday().num_days_from_monday())&&input.month_day.is_none(),"monthly"=>days.is_empty()&&input.month_day.is_some_and(|d|(1..=31).contains(&d))&&matches_date("monthly",&[],input.month_day,start),_=>false};
 if !rule_valid{return Err(bad("The start date must match the chosen recurrence"))}
 let out=db_job(s.clone(),move|c|{
  let canonical=json!({"action":"create_recurrence","data":input});let tx=c.transaction()?;
  let prior:Option<(String,String)>=tx.query_row("SELECT input,result FROM app_commands WHERE device_id=?1 AND request_id=?2",params![input.device_id,input.request_id],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
  if let Some((old,result))=prior{if old!=canonical.to_string(){return Err(conflict("Request ID already used for different data"))}return serde_json::from_str(&result).map_err(|e|ApiError(StatusCode::INTERNAL_SERVER_ERROR,e.to_string()))}
  let task=get_item(&tx,input.task_id)?;if task["kind"]!="task"||!matches!(task["status"].as_str(),Some("inbox"|"ready"))||task["version"].as_i64()!=Some(input.expected_task_version){return Err(conflict("Task changed or cannot become a recurring series"))}
  let has:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM app_task_reminders WHERE task_id=?1)",[input.task_id],|r|r.get(0))?;if has{return Err(conflict("Remove or resolve the current reminder before creating a recurrence"))}
  tx.execute("INSERT INTO app_recurring_rules(template_task_id,frequency,weekdays,month_day,local_time,starts_on) VALUES(?1,?2,?3,?4,?5,?6)",params![input.task_id,input.frequency,serde_json::to_string(&days).unwrap(),input.month_day,input.local_time,input.starts_on])?;let id=tx.last_insert_rowid();
  let at=due_at(scheduled_date(start),&input.local_time).unwrap();tx.execute("UPDATE app_items SET version=version+1,updated_at=CURRENT_TIMESTAMP WHERE id=?1 AND version=?2",params![input.task_id,input.expected_task_version])?;
  tx.execute("INSERT INTO app_recurring_occurrences(rule_id,occurrence_date,task_id,due_at) VALUES(?1,?2,?3,?4)",params![id,input.starts_on,input.task_id,at])?;
  tx.execute("INSERT INTO app_task_reminders(task_id,due_at,next_fire) VALUES(?1,?2,?2)",params![input.task_id,at])?;
  let result=json!({"id":id,"template_task_id":input.task_id,"frequency":input.frequency,"weekdays":days,"month_day":input.month_day,"local_time":input.local_time,"starts_on":input.starts_on,"status":"active","version":0,"first_due_at":at});
  tx.execute("INSERT INTO app_commands(device_id,request_id,input,result) VALUES(?1,?2,?3,?4)",params![input.device_id,input.request_id,canonical.to_string(),result.to_string()])?;tx.commit()?;Ok(result)
 }).await?;s.revision.send_modify(|v|*v+=1);Ok(out)
}

#[derive(Deserialize,Serialize)]pub struct SetRuleStatus{device_id:String,request_id:String,expected_version:i64,status:String}
pub async fn set_status(State(s):State<AppState>,Path(id):Path<i64>,Json(input):Json<SetRuleStatus>)->ApiResult<Value>{
 if input.device_id.is_empty()||input.request_id.is_empty()||input.expected_version<0||!matches!(input.status.as_str(),"active"|"paused"|"ended"){return Err(bad("Invalid recurrence status"))}
 let out=db_job(s.clone(),move|c|{
  let canonical=json!({"action":"set_recurrence_status","id":id,"data":input}).to_string();let tx=c.transaction()?;
  let prior:Option<(String,String)>=tx.query_row("SELECT input,result FROM app_commands WHERE device_id=?1 AND request_id=?2",params![input.device_id,input.request_id],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
  if let Some((old,result))=prior{if old!=canonical{return Err(conflict("Request ID already used for different data"))}return serde_json::from_str(&result).map_err(|e|ApiError(StatusCode::INTERNAL_SERVER_ERROR,e.to_string()))}
  let before:Option<(String,i64)>=tx.query_row("SELECT status,version FROM app_recurring_rules WHERE id=?1",[id],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;let(status,version)=before.ok_or_else(||ApiError(StatusCode::NOT_FOUND,"Recurrence rule not found".into()))?;
  if version!=input.expected_version||status=="ended"&&input.status!="ended"{return Err(conflict("Rule changed or has ended"))}
  let today=today_local(chrono::Utc::now().timestamp()).to_string();
  tx.execute("UPDATE app_recurring_rules SET status=?1,resume_after=CASE WHEN status='paused' AND ?1='active' THEN ?2 ELSE resume_after END,version=version+1 WHERE id=?3 AND version=?4",params![input.status,today,id,input.expected_version])?;
  let result=json!({"id":id,"status":input.status,"version":input.expected_version+1});tx.execute("INSERT INTO app_commands(device_id,request_id,input,result) VALUES(?1,?2,?3,?4)",params![input.device_id,input.request_id,canonical,result.to_string()])?;tx.commit()?;Ok(result)
 }).await?;s.revision.send_modify(|v|*v+=1);Ok(out)
}

pub async fn worker(s:AppState){loop{
 tokio::time::sleep(std::time::Duration::from_secs(60)).await;
 let now=chrono::Utc::now().timestamp();let changed={match s.db.lock(){Ok(mut c)=>match generate(&mut c,now){Ok(n)=>n,Err(e)=>{eprintln!("Recurrence generation failed: {}",e.1);0}},Err(_)=>0}};
 if changed>0{s.revision.send_modify(|v|*v+=1);}
}}

#[cfg(test)]mod tests{
 use super::*;
 fn fixture()->Connection{let c=Connection::open_in_memory().unwrap();super::super::product::schema(&c).unwrap();super::super::task_reminders::schema(&c).unwrap();schema(&c).unwrap();c}
 fn add_task(c:&Connection,id:i64,title:&str){c.execute("INSERT INTO app_items(id,kind,content,original_content,status) VALUES(?1,'task',?2,?2,'ready')",params![id,title]).unwrap();}
 fn local_midnight(date:&str)->i64{let date=NaiveDate::parse_from_str(date,"%Y-%m-%d").unwrap();due_at(date,"00:00").unwrap()}
 #[test]fn daily_rule_keeps_missed_weekdays_and_generates_one_next_occurrence_idempotently(){
  let mut c=fixture();add_task(&c,1,"Tập thể dục");c.execute("INSERT INTO app_recurring_rules(id,template_task_id,frequency,weekdays,local_time,starts_on) VALUES(1,1,'daily','[]','09:00','2026-11-10')",[]).unwrap();
  assert_eq!(generate(&mut c,local_midnight("2026-11-10")).unwrap(),2);
  assert_eq!(c.query_row("SELECT group_concat(occurrence_date,',') FROM app_recurring_occurrences WHERE rule_id=1",[],|r|r.get::<_,String>(0)).unwrap(),"2026-11-10,2026-11-11");
  assert_eq!(generate(&mut c,local_midnight("2026-11-10")).unwrap(),0);
  assert_eq!(generate(&mut c,local_midnight("2026-11-16")).unwrap(),4);
  let dates:Vec<String>={let mut q=c.prepare("SELECT occurrence_date FROM app_recurring_occurrences WHERE rule_id=1 ORDER BY occurrence_date").unwrap();q.query_map([],|r|r.get(0)).unwrap().collect::<Result<_,_>>().unwrap()};
  assert_eq!(dates,vec!["2026-11-10","2026-11-11","2026-11-12","2026-11-13","2026-11-16","2026-11-17"]);
  assert_eq!(c.query_row("SELECT count(*) FROM app_recurring_occurrences WHERE rule_id=1 AND strftime('%w',occurrence_date) IN ('0','6')",[],|r|r.get::<_,i64>(0)).unwrap(),0);
 }
 #[test]fn monthly_day_31_clamps_short_month_and_moves_weekend_to_monday(){
  let mut c=fixture();add_task(&c,1,"Dọn dẹp thiết bị");c.execute("INSERT INTO app_recurring_rules(id,template_task_id,frequency,weekdays,month_day,local_time,starts_on) VALUES(1,1,'monthly','[]',31,'09:00','2026-01-31')",[]).unwrap();
  assert_eq!(generate(&mut c,local_midnight("2026-01-31")).unwrap(),2);
  let (date,at):(String,i64)=c.query_row("SELECT occurrence_date,due_at FROM app_recurring_occurrences WHERE occurrence_date='2026-02-28'",[],|r|Ok((r.get(0)?,r.get(1)?))).unwrap();
  assert_eq!(date,"2026-02-28");assert_eq!(at,due_at(NaiveDate::from_ymd_opt(2026,3,2).unwrap(),"09:00").unwrap());
 }
}
