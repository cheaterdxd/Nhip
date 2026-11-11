//! Manual busy blocks for the personal calendar.
use super::*;

pub fn schema(c:&Connection)->rusqlite::Result<()>{c.execute_batch("CREATE TABLE IF NOT EXISTS app_busy_events(
 id INTEGER PRIMARY KEY,
 title TEXT NOT NULL CHECK(length(title) BETWEEN 1 AND 200),
 starts_at INTEGER NOT NULL,
 ends_at INTEGER NOT NULL CHECK(ends_at>starts_at),
 version INTEGER NOT NULL DEFAULT 0,
 created_at INTEGER NOT NULL
 );
 CREATE INDEX IF NOT EXISTS app_busy_events_time ON app_busy_events(starts_at,ends_at);")?;
 let columns={let mut stmt=c.prepare("PRAGMA table_info(app_busy_events)")?;let values=stmt.query_map([],|r|r.get::<_,String>(1))?.collect::<Result<Vec<_>,_>>()?;values};
 if !columns.iter().any(|column|column=="version"){c.execute_batch("ALTER TABLE app_busy_events ADD COLUMN version INTEGER NOT NULL DEFAULT 0;")?;}
 Ok(())}

#[derive(Deserialize)]
pub struct EventQuery{start:i64,end:i64}
#[derive(Deserialize,Serialize)]
pub struct NewEvent{device_id:String,request_id:String,title:String,starts_at:i64,ends_at:i64}
#[derive(Deserialize)]
pub struct UpdateEvent{expected_version:i64,title:String,starts_at:i64,ends_at:i64}

pub async fn list(State(state):State<AppState>,Query(q):Query<EventQuery>)->ApiResult<Vec<Value>>{
 if q.end<=q.start||q.end-q.start>60*60*24*60{return Err(ApiError(StatusCode::BAD_REQUEST,"Invalid event range".into()))}
 db_job(state,move|c|{
  let mut stmt=c.prepare("SELECT id,title,starts_at,ends_at,version FROM app_busy_events WHERE starts_at<?2 AND ends_at>?1 ORDER BY starts_at")?;
  let rows=stmt.query_map(params![q.start,q.end],|r|Ok(json!({"id":r.get::<_,i64>(0)?,"title":r.get::<_,String>(1)?,"starts_at":r.get::<_,i64>(2)?,"ends_at":r.get::<_,i64>(3)?,"version":r.get::<_,i64>(4)?})))?.collect::<Result<Vec<_>,_>>()?;
  Ok(rows)
 }).await
}

pub async fn create(State(state):State<AppState>,Json(input):Json<NewEvent>)->ApiResult<Value>{
 let title=input.title.trim().to_owned();
 if input.device_id.is_empty()||input.device_id.len()>100||input.request_id.is_empty()||input.request_id.len()>100||title.is_empty()||title.chars().count()>200||input.ends_at<=input.starts_at||input.ends_at-input.starts_at>86400{return Err(ApiError(StatusCode::BAD_REQUEST,"Invalid busy event".into()))}
 db_job(state,move|c|{
  let canonical=json!({"action":"create_busy_event","title":title,"starts_at":input.starts_at,"ends_at":input.ends_at}).to_string();
  let tx=c.transaction()?;
  let prior:Option<(String,String)>=tx.query_row("SELECT input,result FROM app_commands WHERE device_id=?1 AND request_id=?2",params![input.device_id,input.request_id],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
  if let Some((old,result))=prior{if old!=canonical{return Err(ApiError(StatusCode::CONFLICT,"Request ID already used for different data".into()))}return serde_json::from_str(&result).map_err(|e|ApiError(StatusCode::INTERNAL_SERVER_ERROR,e.to_string()))}
  tx.execute("INSERT INTO app_busy_events(title,starts_at,ends_at,created_at) VALUES(?1,?2,?3,?4)",params![title,input.starts_at,input.ends_at,chrono::Utc::now().timestamp()])?;
  let result=json!({"id":tx.last_insert_rowid(),"title":title,"starts_at":input.starts_at,"ends_at":input.ends_at});
  tx.execute("INSERT INTO app_commands(device_id,request_id,input,result) VALUES(?1,?2,?3,?4)",params![input.device_id,input.request_id,canonical,result.to_string()])?;
  tx.commit()?;Ok(result)
 }).await
}

pub async fn delete(State(state):State<AppState>,Path(id):Path<i64>)->ApiResult<Value>{
 db_job(state,move|c|{let count=c.execute("DELETE FROM app_busy_events WHERE id=?1",[id])?;if count==0{return Err(ApiError(StatusCode::NOT_FOUND,"Busy event not found".into()))}Ok(json!({"deleted":id}))}).await
}

pub async fn update(State(state):State<AppState>,Path(id):Path<i64>,Json(input):Json<UpdateEvent>)->ApiResult<Value>{
 let title=input.title.trim().to_owned();
 if title.is_empty()||title.chars().count()>200||input.ends_at<=input.starts_at||input.ends_at-input.starts_at>86400{return Err(ApiError(StatusCode::BAD_REQUEST,"Invalid busy event".into()))}
 db_job(state,move|c|{
  let count=c.execute("UPDATE app_busy_events SET title=?1,starts_at=?2,ends_at=?3,version=version+1 WHERE id=?4 AND version=?5",params![title,input.starts_at,input.ends_at,id,input.expected_version])?;
  if count==0{let exists:bool=c.query_row("SELECT EXISTS(SELECT 1 FROM app_busy_events WHERE id=?1)",[id],|r|r.get(0))?;return Err(ApiError(if exists{StatusCode::CONFLICT}else{StatusCode::NOT_FOUND},if exists{"Busy event changed; refresh before retrying"}else{"Busy event not found"}.into()))}
  Ok(json!({"id":id,"title":title,"starts_at":input.starts_at,"ends_at":input.ends_at,"version":input.expected_version+1}))
 }).await
}
