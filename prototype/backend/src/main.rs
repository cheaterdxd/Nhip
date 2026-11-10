mod reminders;
use axum::{extract::{Path, Query, State}, http::{HeaderValue, Method, StatusCode}, response::{IntoResponse, Response}, routing::{get, post, patch}, Json, Router};
use chrono::{Datelike, Duration, NaiveDate};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{path::PathBuf, sync::{Arc, Mutex}, time::Instant};
use tower_http::{cors::CorsLayer, services::{ServeDir, ServeFile}};

type Db = Arc<Mutex<Connection>>;
type ApiResult<T> = Result<Json<T>, ApiError>;
#[derive(Clone)]
struct AppState { db: Db, revision: tokio::sync::watch::Sender<u64> }
#[derive(Debug)]
struct ApiError(StatusCode, String);
impl IntoResponse for ApiError {
    fn into_response(self) -> Response { (self.0, Json(json!({"error":self.1}))).into_response() }
}
impl From<rusqlite::Error> for ApiError {
    fn from(e: rusqlite::Error) -> Self { Self(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()) }
}
async fn db_job<T: Send + 'static>(state: AppState, job: impl FnOnce(&mut Connection) -> Result<T, ApiError> + Send + 'static) -> ApiResult<T> {
    tokio::task::spawn_blocking(move || {
        let mut conn=state.db.lock().map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR,"Database lock poisoned".into()))?;
        job(&mut conn).map(Json)
    }).await.map_err(|e|ApiError(StatusCode::INTERNAL_SERVER_ERROR,e.to_string()))?
}
#[derive(Serialize)]
struct Project { id:i64, name:String, area:String }
#[derive(Serialize, Clone)]
struct Task {
    id:i64, project_id:i64, project_name:String, area:String, title:String, notes:String,
    duration:i64, remaining:i64, date:Option<String>, minute:Option<i64>, status:String, energy:String, version:i64,
}
fn row_task(row:&rusqlite::Row<'_>)->rusqlite::Result<Task>{
    Ok(Task{id:row.get(0)?,project_id:row.get(1)?,project_name:row.get(2)?,area:row.get(3)?,title:row.get(4)?,notes:row.get(5)?,duration:row.get(6)?,remaining:row.get(7)?,date:row.get(8)?,minute:row.get(9)?,status:row.get(10)?,energy:row.get(11)?,version:row.get(12)?})
}
const TASK_SELECT:&str="SELECT t.id,t.project_id,p.name,p.area,t.title,t.notes,t.duration,t.remaining,t.day,t.minute,t.status,t.energy,t.version FROM tasks t JOIN projects p ON p.id=t.project_id";
fn load_task(conn:&Connection,id:i64)->Result<Task,ApiError>{
    conn.query_row(&format!("{TASK_SELECT} WHERE t.id=?1"),[id],row_task).optional()?.ok_or_else(||ApiError(StatusCode::NOT_FOUND,"Task not found".into()))
}
fn setup(path:&PathBuf)->Result<Connection,Box<dyn std::error::Error>>{
    if let Some(parent)=path.parent(){ std::fs::create_dir_all(parent)?; }
    let mut conn=Connection::open(path)?;
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; PRAGMA foreign_keys=ON;
      CREATE TABLE IF NOT EXISTS projects(id INTEGER PRIMARY KEY,name TEXT NOT NULL,area TEXT NOT NULL CHECK(area IN ('company','personal')));
      CREATE TABLE IF NOT EXISTS tasks(id INTEGER PRIMARY KEY AUTOINCREMENT,project_id INTEGER NOT NULL REFERENCES projects(id),title TEXT NOT NULL,notes TEXT NOT NULL DEFAULT '',duration INTEGER NOT NULL,remaining INTEGER NOT NULL,day TEXT,minute INTEGER,status TEXT NOT NULL,energy TEXT NOT NULL DEFAULT 'medium',version INTEGER NOT NULL DEFAULT 0);
      CREATE INDEX IF NOT EXISTS task_day_status ON tasks(day,status,minute);
      CREATE INDEX IF NOT EXISTS task_status_day ON tasks(status,day);
      CREATE INDEX IF NOT EXISTS task_project ON tasks(project_id,status);
      CREATE TABLE IF NOT EXISTS sessions(id INTEGER PRIMARY KEY AUTOINCREMENT,task_id INTEGER NOT NULL REFERENCES tasks(id),seconds INTEGER NOT NULL,created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP);")?;
    reminders::schema(&conn)?;
    let count:i64=conn.query_row("SELECT count(*) FROM projects",[],|r|r.get(0))?;
    if count==0{
        let tx=conn.transaction()?;
        for id in 1..=100{
            let name=match id{1=>"App quản lý công việc".into(),2=>"Website khách hàng".into(),3=>"Học & phát triển".into(),_=>format!("Dự án {id:03}")};
            tx.execute("INSERT INTO projects(id,name,area) VALUES(?1,?2,?3)",params![id,name,if id%2==0{"company"}else{"personal"}])?;
        }
        let anchor=NaiveDate::from_ymd_opt(2026,11,9).unwrap();
        let titles=["Phác thảo luồng ghi nhanh","Kiểm tra bản thiết kế","Rà soát kế hoạch tuần","Hoàn thiện tài liệu","Ôn lại kiến thức","Chuẩn bị bước tiếp theo"];
        {
            let mut stmt=tx.prepare("INSERT INTO tasks(id,project_id,title,notes,duration,remaining,day,minute,status,energy) VALUES(?1,?2,?3,?4,?5,?5,?6,?7,?8,?9)")?;
            for i in 0..10_000_i64{
                let active=i>=9500;
                let n=if active{i-9500}else{i};
                let offset=if active{n%28}else{-(n%180)-1};
                let date=anchor+Duration::days(offset);
                let project_id=i%100+1;
                let minute=if project_id%2==0{510+(n/28%7)*60}else{1140+(n/28%3)*60};
                stmt.execute(params![i+1,project_id,format!("{} · {}",titles[i as usize%titles.len()],i+1),"Dữ liệu kiểm thử tổng hợp, không phải công việc thật.",25+(i%3)*10,date.to_string(),minute,if active{"ready"}else{"done"},["low","medium","high"][i as usize%3]])?;
            }
        }
        // Historical focus sessions for a non-empty aggregate report, in one seed transaction.
        tx.execute("INSERT INTO sessions(task_id,seconds,created_at) SELECT id,duration*48,day||' 10:00:00' FROM tasks WHERE status='done' AND id%3=0",[])?;
        tx.commit()?;
    }
    Ok(conn)
}
async fn health(State(state):State<AppState>)->ApiResult<Value>{db_job(state,|conn|{
    let (projects,tasks):(i64,i64)=conn.query_row("SELECT (SELECT count(*) FROM projects),(SELECT count(*) FROM tasks)",[],|r|Ok((r.get(0)?,r.get(1)?)))?;
    Ok(json!({"ok":true,"projects":projects,"occurrences":tasks,"fixture_date":"2026-11-09","prototype":true}))
}).await}
async fn projects(State(state):State<AppState>)->ApiResult<Vec<Project>>{db_job(state,|conn|{
    let mut q=conn.prepare("SELECT id,name,area FROM projects ORDER BY id")?;
    let result=q.query_map([],|r|Ok(Project{id:r.get(0)?,name:r.get(1)?,area:r.get(2)?}))?.collect::<Result<Vec<_>,_>>()?;
    Ok(result)
}).await}
#[derive(Deserialize)]
struct TasksQuery{date:Option<String>,limit:Option<i64>,project_id:Option<i64>}
async fn tasks(State(state):State<AppState>,Query(q):Query<TasksQuery>)->ApiResult<Vec<Task>>{db_job(state,move|conn|{
    let day=q.date.unwrap_or_else(||"2026-11-09".into());
    let mut stmt=conn.prepare(&format!("{TASK_SELECT} WHERE t.status='ready' AND (t.day=?1 OR t.day IS NULL) AND (?2 IS NULL OR t.project_id=?2) ORDER BY t.minute IS NULL,t.minute,t.id LIMIT ?3"))?;
    let result=stmt.query_map(params![day,q.project_id,q.limit.unwrap_or(100).clamp(1,200)],row_task)?.collect::<Result<Vec<_>,_>>()?;
    Ok(result)
}).await}
#[derive(Deserialize)]
struct NewTask{project_id:i64,title:String,notes:Option<String>,duration:i64,energy:Option<String>}
async fn create_task(State(state):State<AppState>,Json(input):Json<NewTask>)->ApiResult<Task>{
    if input.title.trim().is_empty()||input.title.chars().count()>240||!(1..=1440).contains(&input.duration){return Err(ApiError(StatusCode::BAD_REQUEST,"Title and duration are invalid".into()))}
    let energy=input.energy.unwrap_or_else(||"medium".into());
    if !["low","medium","high"].contains(&energy.as_str()){return Err(ApiError(StatusCode::BAD_REQUEST,"Invalid energy".into()))}
    db_job(state,move|conn|{
        let exists:bool=conn.query_row("SELECT EXISTS(SELECT 1 FROM projects WHERE id=?1)",[input.project_id],|r|r.get(0))?;
        if !exists{return Err(ApiError(StatusCode::BAD_REQUEST,"Unknown project".into()))}
        conn.execute("INSERT INTO tasks(project_id,title,notes,duration,remaining,status,energy) VALUES(?1,?2,?3,?4,?4,'ready',?5)",params![input.project_id,input.title.trim(),input.notes.unwrap_or_default(),input.duration,energy])?;
        load_task(conn,conn.last_insert_rowid())
    }).await
}
#[derive(Deserialize)]
struct CompleteInput{expected_version:i64}
async fn complete(State(state):State<AppState>,Path(id):Path<i64>,Json(input):Json<CompleteInput>)->ApiResult<Task>{db_job(state,move|conn|{
    let rows=conn.execute("UPDATE tasks SET status='done',version=version+1 WHERE id=?1 AND version=?2 AND status='ready'",params![id,input.expected_version])?;
    if rows==0{return Err(ApiError(StatusCode::CONFLICT,"Task changed; refresh before retrying".into()))}
    load_task(conn,id)
}).await}
#[derive(Deserialize)]
struct CalendarQuery{start:Option<String>}
async fn calendar(State(state):State<AppState>,Query(q):Query<CalendarQuery>)->ApiResult<Vec<Task>>{db_job(state,move|conn|{
    let start=q.start.unwrap_or_else(||"2026-11-09".into());
    let end=NaiveDate::parse_from_str(&start,"%Y-%m-%d").map_err(|_|ApiError(StatusCode::BAD_REQUEST,"Invalid date".into()))?+Duration::days(7);
    let mut stmt=conn.prepare(&format!("{TASK_SELECT} WHERE t.status='ready' AND t.day>=?1 AND t.day<?2 AND t.minute IS NOT NULL ORDER BY t.day,t.minute LIMIT 1000"))?;
    let result=stmt.query_map(params![start,end.to_string()],row_task)?.collect::<Result<Vec<_>,_>>()?;
    Ok(result)
}).await}
#[derive(Deserialize)]
struct SlotQuery{task_id:i64,start:Option<String>}
#[derive(Serialize)]
struct Slot{date:String,minute:i64,duration:i64}
#[derive(Serialize)]
struct SlotResult{task_id:i64,slots:Vec<Slot>,reason:String,server_compute_ms:f64}
fn calculate_slots(conn:&Connection,id:i64,start:&str)->Result<SlotResult,ApiError>{
    let at=Instant::now();let task=load_task(conn,id)?;
    let start=NaiveDate::parse_from_str(start,"%Y-%m-%d").map_err(|_|ApiError(StatusCode::BAD_REQUEST,"Invalid date".into()))?;
    let duration=task.remaining;
    if duration>50{return Ok(SlotResult{task_id:id,slots:vec![],reason:"Prototype only proposes a single session of up to 50 minutes".into(),server_compute_ms:at.elapsed().as_secs_f64()*1000.0})}
    let mut result=Vec::new();
    for offset in 0..7{
        let day=start+Duration::days(offset);let weekday=day.weekday().num_days_from_monday();
        let company=task.area=="company";if company&&weekday>=5{continue}
        let windows:Vec<(i64,i64)>=if company{vec![(510,720),(810,1080)]}else if weekday<5{vec![(420,510),(720,810),(1080,1440)]}else{vec![(420,1440)]};
        let mut stmt=conn.prepare("SELECT t.minute,t.remaining,p.area FROM tasks t JOIN projects p ON p.id=t.project_id WHERE t.day=?1 AND t.status='ready' AND t.minute IS NOT NULL AND t.id!=?2")?;
        let busy=stmt.query_map(params![day.to_string(),id],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,i64>(1)?,r.get::<_,String>(2)?)))?.collect::<Result<Vec<_>,_>>()?;
        let load:i64=busy.iter().filter(|x|x.2==task.area).map(|x|x.1).sum();
        let capacity=if company{300}else if weekday<5{120}else{240};if load+duration>capacity{continue}
        'window: for (a,b) in windows{
            let mut minute=a;
            while minute+duration<=b{
                if busy.iter().all(|x|minute+duration+10<=x.0||minute>=x.0+x.1+10){
                    result.push(Slot{date:day.to_string(),minute,duration});break 'window
                }
                minute+=5;
            }
        }
        if result.len()==3{break}
    }
    let reason=if result.is_empty(){"Không đủ chỗ trong giờ và sức chứa đã đặt"}else{"Giữ đúng giờ công ty/cá nhân, sức chứa và khoảng nghỉ 10 phút"};
    Ok(SlotResult{task_id:id,slots:result,reason:reason.into(),server_compute_ms:at.elapsed().as_secs_f64()*1000.0})
}
async fn slots(State(state):State<AppState>,Query(q):Query<SlotQuery>)->ApiResult<SlotResult>{db_job(state,move|conn|calculate_slots(conn,q.task_id,&q.start.unwrap_or_else(||"2026-11-09".into()))).await}
#[derive(Deserialize)]
struct SessionInput{task_id:i64,seconds:i64}
async fn session(State(state):State<AppState>,Json(input):Json<SessionInput>)->ApiResult<Value>{
    if !(1..=86400).contains(&input.seconds){return Err(ApiError(StatusCode::BAD_REQUEST,"Invalid session duration".into()))}
    db_job(state,move|conn|{load_task(conn,input.task_id)?;conn.execute("INSERT INTO sessions(task_id,seconds) VALUES(?1,?2)",params![input.task_id,input.seconds])?;Ok(json!({"saved":true,"id":conn.last_insert_rowid()}))}).await
}
async fn report(State(state):State<AppState>)->ApiResult<Value>{db_job(state,|conn|{
    let mut stmt=conn.prepare("SELECT status,count(*),sum(duration) FROM tasks GROUP BY status")?;
    let counts=stmt.query_map([],|r|Ok(json!({"status":r.get::<_,String>(0)?,"count":r.get::<_,i64>(1)?,"planned_minutes":r.get::<_,i64>(2)?})))?.collect::<Result<Vec<_>,_>>()?;
    let (sessions,seconds):(i64,i64)=conn.query_row("SELECT count(*),coalesce(sum(seconds),0) FROM sessions",[],|r|Ok((r.get(0)?,r.get(1)?)))?;
    Ok(json!({"scope":"all fixture data (not a production weekly report)","counts":counts,"sessions":sessions,"focused_seconds":seconds}))
}).await}

#[tokio::main(flavor="multi_thread",worker_threads=2)]
async fn main()->Result<(),Box<dyn std::error::Error>>{
    let path=PathBuf::from(std::env::var("NHIP_DB").unwrap_or_else(|_|".data/prototype.sqlite".into()));
    let db=setup(&path)?;
    let state=AppState{db:Arc::new(Mutex::new(db)),revision:tokio::sync::watch::channel(0).0};
    tokio::spawn(reminders::worker(state.clone()));
    let origins=["http://127.0.0.1:1420","http://localhost:1420","http://tauri.localhost","tauri://localhost"].map(|s|s.parse::<HeaderValue>().unwrap());
    let cors=CorsLayer::new().allow_origin(origins).allow_methods([Method::GET,Method::POST,Method::PATCH]).allow_headers([axum::http::header::CONTENT_TYPE,axum::http::header::AUTHORIZATION]);
    let dist=PathBuf::from(std::env::var("NHIP_DIST").unwrap_or_else(|_|"dist".into()));
    let app=Router::new()
      .route("/api/devices",post(reminders::register))
      .route("/api/devices/{device}/updates",get(reminders::updates))
      .route("/api/reminders",post(reminders::create))
      .route("/api/reminders/{id}/ack",post(reminders::ack))
      .route("/api/push/config",get(reminders::config))
      .route("/api/tasks/{id}",get(task_detail))
      .route("/api/health",get(health)).route("/api/projects",get(projects))
      .route("/api/tasks",get(tasks).post(create_task)).route("/api/tasks/{id}/complete",patch(complete))
      .route("/api/calendar",get(calendar)).route("/api/slots",get(slots)).route("/api/sessions",post(session)).route("/api/report",get(report))
      .fallback_service(ServeDir::new(&dist).not_found_service(ServeFile::new(dist.join("index.html"))))
      .layer(axum::middleware::from_fn(access)).layer(cors).with_state(state);
    let port=std::env::var("NHIP_PORT").unwrap_or_else(|_|"4876".into());
    let listener=tokio::net::TcpListener::bind(format!("127.0.0.1:{port}")).await?;
    println!("Nhịp prototype backend http://127.0.0.1:{port} · {}",path.display());
    axum::serve(listener,app).with_graceful_shutdown(async{let _=tokio::signal::ctrl_c().await;}).await?;
    Ok(())
}

#[cfg(test)]
mod tests{
    use super::*;
    fn db()->Connection{let path=std::env::temp_dir().join(format!("nhip-test-{}-{}.sqlite",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));setup(&path).unwrap()}
    #[test] fn fixture_and_planner_respect_constraints(){
        let conn=db();assert_eq!(conn.query_row("SELECT count(*) FROM tasks",[],|r|r.get::<_,i64>(0)).unwrap(),10000);
        let slots=calculate_slots(&conn,9501,"2026-11-09").unwrap();
        for s in slots.slots{assert!(s.minute>=420&&s.minute+s.duration<=1440);let day=NaiveDate::parse_from_str(&s.date,"%Y-%m-%d").unwrap();if day.weekday().num_days_from_monday()<5{assert!(s.minute+s.duration<=510||(s.minute>=720&&s.minute+s.duration<=810)||s.minute>=1080)}}
    }
    #[test] fn stale_completion_cannot_apply_twice(){let conn=db();let sql="UPDATE tasks SET status='done',version=version+1 WHERE id=9501 AND version=0 AND status='ready'";assert_eq!(conn.execute(sql,[]).unwrap(),1);assert_eq!(conn.execute(sql,[]).unwrap(),0);}
    #[test] fn empty_future_week_produces_real_slots(){
        let conn=db();let result=calculate_slots(&conn,9501,"2027-01-04").unwrap();
        assert_eq!(result.slots.len(),3);
        assert_eq!(result.slots[0].minute,420);
        let company=calculate_slots(&conn,9502,"2027-01-09").unwrap();
        assert_eq!(company.slots[0].date,"2027-01-11");
        assert_eq!(company.slots[0].minute,510);
    }
}

async fn task_detail(State(s):State<AppState>,Path(id):Path<i64>)->ApiResult<Task>{db_job(s,move|c|load_task(c,id)).await}
async fn access(req:axum::extract::Request,next:axum::middleware::Next)->Response {
 if req.uri().path().starts_with("/api/") {
  if let Ok(key)=std::env::var("NHIP_ACCESS_KEY") {
   if key.is_empty() || req.headers().get("authorization").and_then(|v|v.to_str().ok())!=Some(format!("Bearer {key}").as_str()) {return (StatusCode::UNAUTHORIZED,Json(json!({"error":"Invalid access key"}))).into_response()}
  }
 }
 next.run(req).await
}
