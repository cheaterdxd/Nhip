use super::*;
use web_push::{ContentEncoding, HyperWebPushClient, SubscriptionInfo, VapidSignatureBuilder, WebPushClient, WebPushMessageBuilder};

pub fn schema(c:&Connection)->rusqlite::Result<()> { c.execute_batch("CREATE TABLE IF NOT EXISTS devices(id TEXT PRIMARY KEY,channel TEXT NOT NULL,subscription TEXT);
CREATE TABLE IF NOT EXISTS reminders(id INTEGER PRIMARY KEY AUTOINCREMENT,request_id TEXT UNIQUE NOT NULL,task_id INTEGER NOT NULL REFERENCES tasks(id),due INTEGER NOT NULL,ack_device TEXT,announced INTEGER NOT NULL DEFAULT 0);
CREATE TABLE IF NOT EXISTS push_delivery(reminder INTEGER,device TEXT,attempts INTEGER NOT NULL DEFAULT 0,next_try INTEGER NOT NULL DEFAULT 0,status TEXT NOT NULL DEFAULT 'pending',PRIMARY KEY(reminder,device));") }
fn bad(s:&str)->ApiError {ApiError(StatusCode::BAD_REQUEST,s.into())}
#[derive(Deserialize)] pub struct Device {id:String,channel:String,subscription:Option<SubscriptionInfo>}
pub async fn register(State(s):State<AppState>,Json(d):Json<Device>)->ApiResult<Value>{
 if d.id.is_empty()||d.id.len()>100||!["desktop","webpush","browser"].contains(&d.channel.as_str()){return Err(bad("Invalid device"))}
 if d.channel=="webpush" {
  let sub=d.subscription.as_ref().ok_or_else(||bad("Missing push subscription"))?;
  let uri:axum::http::Uri=sub.endpoint.parse().map_err(|_|bad("Invalid push URL"))?;
  let h=uri.host().unwrap_or("");
  if uri.scheme_str()!=Some("https")||uri.port_u16().is_some_and(|p|p!=443)||!(h=="web.push.apple.com"||h.ends_with(".push.apple.com")||h=="fcm.googleapis.com"||h=="updates.push.services.mozilla.com"){return Err(bad("Unsupported push service"))}
 }
 let out=db_job(s.clone(),move|c|{c.execute("INSERT INTO devices VALUES(?1,?2,?3) ON CONFLICT(id) DO UPDATE SET channel=excluded.channel,subscription=excluded.subscription",params![d.id,d.channel,d.subscription.map(|x|serde_json::to_string(&x).unwrap())])?;Ok(json!({"registered":true}))}).await?;
 s.revision.send_modify(|v|*v+=1);Ok(out)
}
#[derive(Deserialize)] pub struct NewReminder {request_id:String,task_id:i64,delay_seconds:i64}
pub async fn create(State(s):State<AppState>,Json(r):Json<NewReminder>)->ApiResult<Value>{
 if r.request_id.is_empty()||r.request_id.len()>100||!(0..=86400).contains(&r.delay_seconds){return Err(bad("Invalid reminder"))}
 let result=db_job(s.clone(),move|c|{
  load_task(c,r.task_id)?;
  let due=chrono::Utc::now().timestamp()+r.delay_seconds;
  c.execute("INSERT INTO reminders(request_id,task_id,due) VALUES(?1,?2,?3) ON CONFLICT(request_id) DO NOTHING",params![r.request_id,r.task_id,due])?;
  let v=c.query_row("SELECT id,task_id FROM reminders WHERE request_id=?1",[r.request_id],|row|Ok((row.get::<_,i64>(0)?,row.get::<_,i64>(1)?)))?;
  if v.1!=r.task_id {return Err(ApiError(StatusCode::CONFLICT,"Request ID reused for another task".into()))}
  Ok(json!({"id":v.0,"task_id":v.1}))
 }).await?;s.revision.send_modify(|v|*v+=1);Ok(result)
}
#[derive(Deserialize)] pub struct Ack {device_id:String}
pub async fn ack(State(s):State<AppState>,Path(id):Path<i64>,Json(a):Json<Ack>)->ApiResult<Value>{
 let result=db_job(s.clone(),move|c|{
  let exists:bool=c.query_row("SELECT EXISTS(SELECT 1 FROM devices WHERE id=?1)",[&a.device_id],|r|r.get(0))?;
  if !exists{return Err(bad("Register device first"))}
  c.execute("UPDATE reminders SET ack_device=?2 WHERE id=?1 AND ack_device IS NULL",params![id,a.device_id])?;
  let ack:Option<String>=c.query_row("SELECT ack_device FROM reminders WHERE id=?1",[id],|r|r.get(0)).optional()?.ok_or_else(||ApiError(StatusCode::NOT_FOUND,"Unknown reminder".into()))?;
  Ok(json!({"id":id,"ack_device":ack}))
 }).await?;s.revision.send_modify(|v|*v+=1);Ok(result)
}
#[derive(Deserialize)] pub struct Poll {since:Option<u64>}
pub async fn updates(State(s):State<AppState>,Path(device):Path<String>,Query(q):Query<Poll>)->ApiResult<Value>{
 let mut rx=s.revision.subscribe();
 if q.since==Some(*rx.borrow_and_update()){let _=tokio::time::timeout(std::time::Duration::from_secs(25),rx.changed()).await;}
 let revision=*rx.borrow();
 db_job(s,move|c|{
  let exists:bool=c.query_row("SELECT EXISTS(SELECT 1 FROM devices WHERE id=?1)",[device],|r|r.get(0))?;
  if !exists{return Err(bad("Unknown device"))}
  // This prototype returns all due reminders so old unresolved items and their
  // acknowledgments cannot disappear behind a fixed newest-first limit.
  let mut stmt=c.prepare("SELECT r.id,r.task_id,t.title,r.ack_device FROM reminders r JOIN tasks t ON t.id=r.task_id WHERE r.due<=?1 ORDER BY r.id DESC")?;
  let mut rows=stmt.query_map([chrono::Utc::now().timestamp()],|r|Ok(json!({"id":r.get::<_,i64>(0)?,"task_id":r.get::<_,i64>(1)?,"title":r.get::<_,String>(2)?,"ack_device":r.get::<_,Option<String>>(3)?,"source":"fixture"})))?.collect::<Result<Vec<_>,_>>()?;
  let mut app=c.prepare("SELECT a.id,r.id,r.task_id,i.content,r.state,r.due_at,r.version,r.snoozes,a.id=COALESCE((SELECT MAX(a2.id) FROM app_reminder_alerts a2 WHERE a2.reminder_id=r.id),0) AND a.id>r.last_reset_alert,a.kind FROM app_reminder_alerts a JOIN app_task_reminders r ON r.id=a.reminder_id JOIN app_items i ON i.id=r.task_id ORDER BY a.id DESC")?;
  let app_rows=app.query_map([],|r|{
   let alert_id:i64=r.get(0)?;let state:String=r.get(4)?;let latest:bool=r.get(8)?;let kind:String=r.get(9)?;
   Ok(json!({"id":1_000_000_000_i64+alert_id,"source":"app","reminder_id":r.get::<_,i64>(1)?,"task_id":r.get::<_,i64>(2)?,"title":r.get::<_,String>(3)?,"kind":kind,"state":state,"due_at":r.get::<_,i64>(5)?,"version":r.get::<_,i64>(6)?,"snoozes":r.get::<_,i64>(7)?,"ack_device":if (state=="scheduled"||state=="decision")&&latest {None} else {Some("resolved")}}))
  })?.collect::<Result<Vec<_>,_>>()?;
  rows.extend(app_rows);
  let mut weekly=c.prepare("SELECT week_start,ack_device FROM app_weekly_report_alerts ORDER BY created_at DESC")?;
  let weekly_rows=weekly.query_map([],|r|{
   let week_start:String=r.get(0)?;
   let id=crate::weekly_report::notification_id(&week_start).unwrap_or(0);
   Ok(json!({"id":id,"task_id":0,"title":"Báo cáo tuần đã sẵn sàng","source":"report","kind":"weekly_report","week_start":week_start,"ack_device":r.get::<_,Option<String>>(1)?}))
  })?.collect::<Result<Vec<_>,_>>()?;
  rows.extend(weekly_rows);
  Ok(json!({"revision":revision,"reminders":rows}))
 }).await
}
pub async fn config()->Json<Value>{Json(json!({"public_key":std::env::var("NHIP_VAPID_PUBLIC").ok(),"push_configured":std::env::var("NHIP_VAPID_PEM").is_ok()}))}
pub async fn worker(s:AppState){
 let client=HyperWebPushClient::new();
 loop {
  tokio::time::sleep(std::time::Duration::from_secs(1)).await;
  let jobs={let c=match s.db.lock(){Ok(c)=>c,Err(_)=>continue};
   let now=chrono::Utc::now().timestamp();
   let report_created=match crate::weekly_report::dispatch_due(&c,now){Ok(created)=>created,Err(e)=>{eprintln!("Weekly report alert scheduling failed: {}",e.1);false}};
   if report_created{s.revision.send_modify(|v|*v+=1);}
   let changed=c.execute("UPDATE reminders SET announced=1 WHERE due<=?1 AND announced=0",[now]).unwrap_or(0);
   if changed>0{s.revision.send_modify(|v|*v+=1);}
   let _=c.execute("INSERT OR IGNORE INTO push_delivery(reminder,device) SELECT r.id,d.id FROM reminders r CROSS JOIN devices d WHERE r.ack_device IS NULL AND r.due<=?1 AND d.channel='webpush'",[chrono::Utc::now().timestamp()]);
   let mut stmt=match c.prepare("SELECT r.id,r.task_id,d.id,d.subscription FROM push_delivery p JOIN reminders r ON r.id=p.reminder JOIN devices d ON d.id=p.device WHERE r.ack_device IS NULL AND p.status='pending' AND p.attempts<5 AND p.next_try<=?1 LIMIT 10"){Ok(x)=>x,Err(_)=>continue};
   let rows=stmt.query_map([chrono::Utc::now().timestamp()],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,i64>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?))).unwrap().collect::<Result<Vec<_>,_>>().unwrap_or_default(); rows
  };
  for (id,task,device,subscription) in jobs {
   let result=async {
    let pem=std::fs::read(std::env::var("NHIP_VAPID_PEM").map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
    let sub:SubscriptionInfo=serde_json::from_str(&subscription).map_err(|e|e.to_string())?;
    let mut sig=VapidSignatureBuilder::from_pem(pem.as_slice(),&sub).map_err(|e|e.to_string())?;
    sig.add_claim("sub",std::env::var("NHIP_VAPID_SUBJECT").unwrap_or_else(|_|"mailto:admin@example.com".into()));
    let payload=json!({"id":id,"task_id":task,"title":"Nhịp · Có công việc cần phản hồi"}).to_string();
    let mut builder=WebPushMessageBuilder::new(&sub);
    builder.set_payload(ContentEncoding::Aes128Gcm,payload.as_bytes());builder.set_vapid_signature(sig.build().map_err(|e|e.to_string())?);
    tokio::time::timeout(std::time::Duration::from_secs(10),client.send(builder.build().map_err(|e|e.to_string())?)).await.map_err(|e|e.to_string())?.map_err(|e|e.to_string())
   }.await;
   if let Ok(c)=s.db.lock(){let _=c.execute("UPDATE push_delivery SET attempts=attempts+1,next_try=?3,status=?4 WHERE reminder=?1 AND device=?2",params![id,device,chrono::Utc::now().timestamp()+60,if result.is_ok(){"sent"}else{"pending"}]);}
   if result.is_err(){eprintln!("Push {id} failed; retry is scheduled.");}
  }
  let app_jobs={let c=match s.db.lock(){Ok(c)=>c,Err(_)=>continue};
   let _=c.execute("INSERT OR IGNORE INTO app_push_delivery(alert_id,device) SELECT a.id,d.id FROM app_reminder_alerts a JOIN app_task_reminders r ON r.id=a.reminder_id CROSS JOIN devices d WHERE r.state IN ('scheduled','decision') AND a.id>r.last_reset_alert AND a.id=(SELECT MAX(a2.id) FROM app_reminder_alerts a2 WHERE a2.reminder_id=r.id) AND d.channel='webpush'",[]);
   let mut stmt=match c.prepare("SELECT a.id,r.task_id,d.id,d.subscription,a.kind FROM app_push_delivery p JOIN app_reminder_alerts a ON a.id=p.alert_id JOIN app_task_reminders r ON r.id=a.reminder_id JOIN devices d ON d.id=p.device WHERE r.state IN ('scheduled','decision') AND a.id>r.last_reset_alert AND a.id=(SELECT MAX(a2.id) FROM app_reminder_alerts a2 WHERE a2.reminder_id=r.id) AND p.status='pending' AND p.attempts<5 AND p.next_try<=?1 LIMIT 10"){Ok(q)=>q,Err(_)=>continue};
   let rows=stmt.query_map([chrono::Utc::now().timestamp()],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,i64>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?,r.get::<_,String>(4)?))).unwrap().collect::<Result<Vec<_>,_>>().unwrap_or_default();rows
  };
  for (alert,task,device,subscription,kind) in app_jobs {
   let result=async {
    let pem=std::fs::read(std::env::var("NHIP_VAPID_PEM").map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
    let sub:SubscriptionInfo=serde_json::from_str(&subscription).map_err(|e|e.to_string())?;
    let mut sig=VapidSignatureBuilder::from_pem(pem.as_slice(),&sub).map_err(|e|e.to_string())?;
    sig.add_claim("sub",std::env::var("NHIP_VAPID_SUBJECT").unwrap_or_else(|_|"mailto:admin@example.com".into()));
    let title=if kind=="decision"{"Cần chọn: làm ngay, tái cấu trúc hoặc bỏ"}else{"Bạn có công việc cần phản hồi"};
    let payload=json!({"id":1_000_000_000_i64+alert,"source":"app","task_id":task,"title":title}).to_string();
    let mut builder=WebPushMessageBuilder::new(&sub);builder.set_payload(ContentEncoding::Aes128Gcm,payload.as_bytes());builder.set_vapid_signature(sig.build().map_err(|e|e.to_string())?);
    tokio::time::timeout(std::time::Duration::from_secs(10),client.send(builder.build().map_err(|e|e.to_string())?)).await.map_err(|e|e.to_string())?.map_err(|e|e.to_string())
   }.await;
   if let Ok(c)=s.db.lock(){let _=c.execute("UPDATE app_push_delivery SET attempts=attempts+1,next_try=?3,status=?4 WHERE alert_id=?1 AND device=?2",params![alert,device,chrono::Utc::now().timestamp()+60,if result.is_ok(){"sent"}else{"pending"}]);}
   if result.is_err(){eprintln!("App push alert {alert} failed; retry is scheduled.");}
  }
  let report_jobs={let c=match s.db.lock(){Ok(c)=>c,Err(_)=>continue};
   let _=c.execute("INSERT OR IGNORE INTO app_weekly_report_delivery(week_start,device) SELECT a.week_start,d.id FROM app_weekly_report_alerts a CROSS JOIN devices d WHERE a.acknowledged_at IS NULL AND d.channel='webpush'",[]);
   let mut stmt=match c.prepare("SELECT a.week_start,d.id,d.subscription FROM app_weekly_report_delivery p JOIN app_weekly_report_alerts a ON a.week_start=p.week_start JOIN devices d ON d.id=p.device WHERE a.acknowledged_at IS NULL AND p.status='pending' AND p.attempts<5 AND p.next_try<=?1 LIMIT 10"){Ok(q)=>q,Err(_)=>continue};
   stmt.query_map([chrono::Utc::now().timestamp()],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?))).unwrap().collect::<Result<Vec<_>,_>>().unwrap_or_default()
  };
  for (week_start,device,subscription) in report_jobs{
   let result=async {
    let pem=std::fs::read(std::env::var("NHIP_VAPID_PEM").map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
    let sub:SubscriptionInfo=serde_json::from_str(&subscription).map_err(|e|e.to_string())?;
    let mut sig=VapidSignatureBuilder::from_pem(pem.as_slice(),&sub).map_err(|e|e.to_string())?;
    sig.add_claim("sub",std::env::var("NHIP_VAPID_SUBJECT").unwrap_or_else(|_|"mailto:admin@example.com".into()));
    let id=crate::weekly_report::notification_id(&week_start).ok_or_else(||"Invalid weekly report date".to_owned())?;
    let payload=json!({"id":id,"task_id":0,"title":"Báo cáo tuần đã sẵn sàng","source":"report","week_start":week_start}).to_string();
    let mut builder=WebPushMessageBuilder::new(&sub);builder.set_payload(ContentEncoding::Aes128Gcm,payload.as_bytes());builder.set_vapid_signature(sig.build().map_err(|e|e.to_string())?);
    tokio::time::timeout(std::time::Duration::from_secs(10),client.send(builder.build().map_err(|e|e.to_string())?)).await.map_err(|e|e.to_string())?.map_err(|e|e.to_string())
   }.await;
   if let Ok(c)=s.db.lock(){let _=c.execute("UPDATE app_weekly_report_delivery SET attempts=attempts+1,next_try=?3,status=?4 WHERE week_start=?1 AND device=?2",params![week_start,device,chrono::Utc::now().timestamp()+60,if result.is_ok(){"sent"}else{"pending"}]);}
   if result.is_err(){eprintln!("Weekly report push failed; retry is scheduled.");}
  }
 }
}
