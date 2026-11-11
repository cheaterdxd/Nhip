//! Calendar views over the app's scheduled tasks and recorded focus sessions.
use super::*;

const LOCAL_OFFSET_SECONDS:i64=7*60*60;

#[derive(Deserialize)]
pub struct CalendarQuery { start:Option<String>,days:Option<i64> }

pub async fn get(State(state):State<AppState>,Query(query):Query<CalendarQuery>)->ApiResult<Value>{
 let start=NaiveDate::parse_from_str(query.start.as_deref().unwrap_or(""),"%Y-%m-%d")
  .map_err(|_|ApiError(StatusCode::BAD_REQUEST,"start must be YYYY-MM-DD".into()))?;
 let days=query.days.unwrap_or(7);
 if !(1..=42).contains(&days){return Err(ApiError(StatusCode::BAD_REQUEST,"days must be between 1 and 42".into()))}
 let following=start+Duration::days(days);
 let from=start.and_hms_opt(0,0,0).unwrap().and_utc().timestamp()-LOCAL_OFFSET_SECONDS;
 let to=following.and_hms_opt(0,0,0).unwrap().and_utc().timestamp()-LOCAL_OFFSET_SECONDS;
 let now=chrono::Utc::now().timestamp();
 db_job(state,move|c|{
  let appointments={let mut q=c.prepare("SELECT r.id,r.task_id,i.content,coalesce(p.name,'Chưa phân loại'),i.project_id,i.duration,i.energy,r.due_at,r.state,r.snoozes,o.rule_id,o.occurrence_date FROM app_task_reminders r JOIN app_items i ON i.id=r.task_id LEFT JOIN app_projects p ON p.id=i.project_id LEFT JOIN app_recurring_occurrences o ON o.task_id=i.id WHERE r.due_at>=?1 AND r.due_at<?2 AND r.state IN ('scheduled','decision') AND NOT EXISTS(SELECT 1 FROM app_schedule_plans sp WHERE sp.task_id=r.task_id AND sp.state='active') ORDER BY r.due_at,i.id")?;
   let rows=q.query_map(params![from,to],|r|Ok(json!({"reminder_id":r.get::<_,i64>(0)?,"task_id":r.get::<_,i64>(1)?,"content":r.get::<_,String>(2)?,"project":r.get::<_,String>(3)?,"project_id":r.get::<_,Option<i64>>(4)?,"duration_minutes":r.get::<_,Option<i64>>(5)?,"energy":r.get::<_,Option<String>>(6)?,"due_at":r.get::<_,i64>(7)?,"state":r.get::<_,String>(8)?,"snoozes":r.get::<_,i64>(9)?,"recurrence_rule_id":r.get::<_,Option<i64>>(10)?,"occurrence_date":r.get::<_,Option<String>>(11)?})))?.collect::<Result<Vec<_>,_>>()?;rows};
  let overdue={let mut q=c.prepare("SELECT r.id,r.task_id,i.content,coalesce(p.name,'Chưa phân loại'),r.due_at,r.state,r.snoozes FROM app_task_reminders r JOIN app_items i ON i.id=r.task_id LEFT JOIN app_projects p ON p.id=i.project_id WHERE r.due_at<?1 AND r.state IN ('scheduled','decision') AND i.status NOT IN ('done','abandoned','restructured') ORDER BY r.due_at")?;
   let rows=q.query_map([now],|r|Ok(json!({"reminder_id":r.get::<_,i64>(0)?,"task_id":r.get::<_,i64>(1)?,"content":r.get::<_,String>(2)?,"project":r.get::<_,String>(3)?,"due_at":r.get::<_,i64>(4)?,"state":r.get::<_,String>(5)?,"snoozes":r.get::<_,i64>(6)?})))?.collect::<Result<Vec<_>,_>>()?;rows};
  let focus_sessions={let mut q=c.prepare("SELECT f.id,f.task_id,i.content,f.started_at,coalesce(f.ended_at,f.started_at+f.planned_seconds),f.actual_seconds,f.state FROM app_focus_sessions f JOIN app_items i ON i.id=f.task_id WHERE f.started_at>=?1 AND f.started_at<?2 ORDER BY f.started_at")?;
   let rows=q.query_map(params![from,to],|r|Ok(json!({"id":r.get::<_,i64>(0)?,"task_id":r.get::<_,i64>(1)?,"content":r.get::<_,String>(2)?,"started_at":r.get::<_,i64>(3)?,"ends_at":r.get::<_,i64>(4)?,"actual_seconds":r.get::<_,Option<i64>>(5)?,"state":r.get::<_,String>(6)?})))?.collect::<Result<Vec<_>,_>>()?;rows};
  let busy_events={let mut q=c.prepare("SELECT id,title,starts_at,ends_at,version FROM app_busy_events WHERE starts_at<?2 AND ends_at>?1 ORDER BY starts_at")?;
   let rows=q.query_map(params![from,to],|r|Ok(json!({"id":r.get::<_,i64>(0)?,"title":r.get::<_,String>(1)?,"starts_at":r.get::<_,i64>(2)?,"ends_at":r.get::<_,i64>(3)?,"version":r.get::<_,i64>(4)?})))?.collect::<Result<Vec<_>,_>>()?;rows};
  let planned_sessions={let mut q=c.prepare("SELECT s.id,p.task_id,i.content,s.starts_at,s.duration_minutes,s.state FROM app_schedule_sessions s JOIN app_schedule_plans p ON p.id=s.plan_id JOIN app_items i ON i.id=p.task_id WHERE (s.state='done' OR (p.state='active' AND s.state='planned' AND i.status NOT IN ('done','abandoned','restructured'))) AND s.starts_at>=?1 AND s.starts_at<?2 ORDER BY s.starts_at")?;
   let rows=q.query_map(params![from,to],|r|Ok(json!({"id":r.get::<_,i64>(0)?,"task_id":r.get::<_,i64>(1)?,"content":r.get::<_,String>(2)?,"starts_at":r.get::<_,i64>(3)?,"duration_minutes":r.get::<_,i64>(4)?,"state":r.get::<_,String>(5)?})))?.collect::<Result<Vec<_>,_>>()?;rows};
  Ok(json!({"start":start.to_string(),"end":(following-Duration::days(1)).to_string(),"appointments":appointments,"overdue":overdue,"focus_sessions":focus_sessions,"busy_events":busy_events,"planned_sessions":planned_sessions}))
 }).await
}
