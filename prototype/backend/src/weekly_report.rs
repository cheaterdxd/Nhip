//! Weekly operational report for the local user's actual app data.
use super::*;

const LOCAL_OFFSET_SECONDS:i64=7*60*60;

pub fn schema(c:&Connection)->rusqlite::Result<()> {
 c.execute_batch("CREATE TABLE IF NOT EXISTS app_weekly_report_alerts(week_start TEXT PRIMARY KEY,created_at INTEGER NOT NULL,acknowledged_at INTEGER,ack_device TEXT);
 CREATE TABLE IF NOT EXISTS app_weekly_report_delivery(week_start TEXT NOT NULL REFERENCES app_weekly_report_alerts(week_start),device TEXT NOT NULL,attempts INTEGER NOT NULL DEFAULT 0,next_try INTEGER NOT NULL DEFAULT 0,status TEXT NOT NULL DEFAULT 'pending',PRIMARY KEY(week_start,device));")
}

pub fn notification_id(week_start:&str)->Option<i64>{
 let monday=NaiveDate::parse_from_str(week_start,"%Y-%m-%d").ok()?;
 let iso=monday.iso_week();
 Some(1_500_000_000_i64+iso.year() as i64*100+iso.week() as i64)
}

/// Create exactly one report alert after 20:00 local time each Sunday.
/// On restart, dispatch the most recently missed Sunday instead of losing it.
pub fn dispatch_due(c:&Connection,now:i64)->Result<bool,ApiError>{
 let local=chrono::DateTime::<chrono::Utc>::from_timestamp(now,0).ok_or_else(||ApiError(StatusCode::INTERNAL_SERVER_ERROR,"Invalid clock".into()))?.naive_utc()+Duration::seconds(LOCAL_OFFSET_SECONDS);
 let sunday=local.date()-Duration::days(local.weekday().num_days_from_sunday() as i64);
 let this_sunday_at_20=sunday.and_hms_opt(20,0,0).unwrap();
 let report_sunday=if local>=this_sunday_at_20{sunday}else{sunday-Duration::days(7)};
 let monday=(report_sunday-Duration::days(6)).to_string();
 let inserted=c.execute("INSERT OR IGNORE INTO app_weekly_report_alerts(week_start,created_at) VALUES(?1,?2)",params![monday,now])?;
 if inserted==1{
  c.execute("INSERT OR IGNORE INTO app_weekly_report_delivery(week_start,device) SELECT ?1,id FROM devices WHERE channel='webpush'",[&monday])?;
  Ok(true)
 }else{Ok(false)}
}

#[derive(Deserialize)]
pub struct WeeklyQuery { week_start:Option<String> }

pub async fn get(State(state):State<AppState>,Query(query):Query<WeeklyQuery>)->ApiResult<Value>{
 let today=chrono::DateTime::<chrono::Utc>::from_timestamp(chrono::Utc::now().timestamp(),0)
  .unwrap().naive_utc().checked_add_signed(Duration::seconds(LOCAL_OFFSET_SECONDS)).unwrap().date();
 let requested=match query.week_start {
  Some(value)=>NaiveDate::parse_from_str(&value,"%Y-%m-%d").map_err(|_|ApiError(StatusCode::BAD_REQUEST,"week_start must be YYYY-MM-DD".into()))?,
  None=>today-Duration::days(today.weekday().num_days_from_monday() as i64),
 };
 if requested.weekday()!=chrono::Weekday::Mon{return Err(ApiError(StatusCode::BAD_REQUEST,"week_start must be a Monday".into()))}
 let following=requested+Duration::days(7);
 let from=requested.and_hms_opt(0,0,0).unwrap().and_utc().timestamp()-LOCAL_OFFSET_SECONDS;
 let to=following.and_hms_opt(0,0,0).unwrap().and_utc().timestamp()-LOCAL_OFFSET_SECONDS;
 let now=chrono::Utc::now().timestamp();
 db_job(state,move|c|{
  let completed={let mut q=c.prepare("SELECT i.id,i.content,CAST(strftime('%s',h.created_at) AS INTEGER) FROM app_item_history h JOIN app_items i ON i.id=h.item_id WHERE h.action='complete' AND CAST(strftime('%s',h.created_at) AS INTEGER)>=?1 AND CAST(strftime('%s',h.created_at) AS INTEGER)<?2 ORDER BY h.created_at")?;
   let rows=q.query_map(params![from,to],|r|Ok(json!({"task_id":r.get::<_,i64>(0)?,"content":r.get::<_,String>(1)?,"completed_at":r.get::<_,i64>(2)?})))?.collect::<Result<Vec<_>,_>>()?;rows};
  let postponements={let mut q=c.prepare("SELECT e.reminder_id,r.task_id,i.content,e.at,e.reason,e.due_at FROM app_reminder_events e JOIN app_task_reminders r ON r.id=e.reminder_id JOIN app_items i ON i.id=r.task_id WHERE e.kind='postpone' AND e.at>=?1 AND e.at<?2 ORDER BY e.at")?;
   let rows=q.query_map(params![from,to],|r|Ok(json!({"reminder_id":r.get::<_,i64>(0)?,"task_id":r.get::<_,i64>(1)?,"content":r.get::<_,String>(2)?,"at":r.get::<_,i64>(3)?,"reason":r.get::<_,Option<String>>(4)?,"due_at":r.get::<_,Option<i64>>(5)?})))?.collect::<Result<Vec<_>,_>>()?;rows};
  let alerts:i64=c.query_row("SELECT count(*) FROM app_reminder_alerts WHERE created_at>=?1 AND created_at<?2",params![from,to],|r|r.get(0))?;
  let focus={let mut q=c.prepare("SELECT f.task_id,i.content,coalesce(sum(CASE WHEN f.actual_seconds IS NOT NULL THEN f.actual_seconds WHEN f.state='active' THEN max(0,min(?3,f.started_at+f.planned_seconds)-f.started_at) ELSE 0 END),0) FROM app_focus_sessions f JOIN app_items i ON i.id=f.task_id WHERE f.started_at>=?1 AND f.started_at<?2 GROUP BY f.task_id,i.content ORDER BY 3 DESC")?;
   let rows=q.query_map(params![from,to,now],|r|Ok(json!({"task_id":r.get::<_,i64>(0)?,"content":r.get::<_,String>(1)?,"focused_seconds":r.get::<_,i64>(2)?})))?.collect::<Result<Vec<_>,_>>()?;rows};
  let overdue:i64=c.query_row("SELECT count(*) FROM app_task_reminders r JOIN app_items i ON i.id=r.task_id WHERE r.due_at<?1 AND r.state IN ('scheduled','decision') AND i.status NOT IN ('done','abandoned','restructured')",[now],|r|r.get(0))?;
  let focused_seconds=focus.iter().map(|v|v["focused_seconds"].as_i64().unwrap_or(0)).sum::<i64>();
  let mut observations=Vec::new();
  if !postponements.is_empty(){observations.push(format!("Tuần này có {} lượt hoãn; xem lại các lý do bên dưới để nhận ra việc nào đang bị kẹt.",postponements.len()));}
  if overdue>0{observations.push(format!("Hiện còn {overdue} task quá hạn chưa đóng; nên chọn hẹn lại, tái cấu trúc hoặc bỏ."));}
  if postponements.is_empty()&&overdue==0{observations.push("Không ghi nhận lượt hoãn trong tuần và hiện không còn task quá hạn.".to_owned());}
  Ok(json!({"week_start":requested.to_string(),"week_end":(following-Duration::days(1)).to_string(),"counts":{"completed":completed.len(),"postponements":postponements.len(),"alerts":alerts,"overdue_now":overdue,"focused_seconds":focused_seconds},"completed_tasks":completed,"postponements":postponements,"focus_by_task":focus,"observations":observations,"analysis_source":"deterministic_app_data","ai_analysis_available":true}))
 }).await
}

#[derive(Deserialize)]
pub struct AcknowledgeReport {device_id:String}
pub async fn acknowledge(State(state):State<AppState>,Path(week_start):Path<String>,Json(input):Json<AcknowledgeReport>)->ApiResult<Value>{
 let date=NaiveDate::parse_from_str(&week_start,"%Y-%m-%d").map_err(|_|ApiError(StatusCode::BAD_REQUEST,"week_start must be YYYY-MM-DD".into()))?;
 if date.weekday()!=chrono::Weekday::Mon||input.device_id.is_empty()||input.device_id.len()>100{return Err(ApiError(StatusCode::BAD_REQUEST,"Invalid report acknowledgment".into()))}
 let now=chrono::Utc::now().timestamp();
 let result=db_job(state.clone(),move|c|{
  c.execute("UPDATE app_weekly_report_alerts SET acknowledged_at=COALESCE(acknowledged_at,?1),ack_device=COALESCE(ack_device,?2) WHERE week_start=?3",params![now,input.device_id,week_start])?;
  let ack:Option<String>=c.query_row("SELECT ack_device FROM app_weekly_report_alerts WHERE week_start=?1",[&week_start],|r|r.get(0)).optional()?;
  Ok(json!({"week_start":week_start,"ack_device":ack}))
 }).await?;
 state.revision.send_modify(|v|*v+=1);
 Ok(result)
}

#[cfg(test)]
mod tests{
 use super::*;
 use chrono::NaiveDateTime;
 fn fixture()->Connection{
  let c=Connection::open_in_memory().unwrap();
  c.execute_batch("CREATE TABLE devices(id TEXT PRIMARY KEY,channel TEXT NOT NULL,subscription TEXT);").unwrap();
  schema(&c).unwrap();c
 }
 fn timestamp(local:&str)->i64{
  NaiveDateTime::parse_from_str(local,"%Y-%m-%d %H:%M:%S").unwrap().and_utc().timestamp()-LOCAL_OFFSET_SECONDS
 }
 #[test]
 fn weekly_alert_is_once_after_sunday_eight_or_caught_up_after_restart(){
  let c=fixture();
  c.execute("INSERT INTO app_weekly_report_alerts(week_start,created_at) VALUES('2026-10-26',0)",[]).unwrap();
  assert!(!dispatch_due(&c,timestamp("2026-11-08 19:59:59")).unwrap());
  assert!(dispatch_due(&c,timestamp("2026-11-08 20:00:00")).unwrap());
  assert!(!dispatch_due(&c,timestamp("2026-11-08 21:00:00")).unwrap());
  assert!(c.query_row("SELECT EXISTS(SELECT 1 FROM app_weekly_report_alerts WHERE week_start='2026-11-02')",[],|r|r.get::<_,bool>(0)).unwrap());
  assert_eq!(notification_id("2026-11-02"),Some(1_500_202_645));

  let restarted=fixture();
  assert!(dispatch_due(&restarted,timestamp("2026-11-09 08:00:00")).unwrap());
  assert_eq!(restarted.query_row("SELECT week_start FROM app_weekly_report_alerts",[],|r|r.get::<_,String>(0)).unwrap(),"2026-11-02");
 }
}
