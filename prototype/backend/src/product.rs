//! Real-data foundation, separate from the performance fixture tables.
use super::*;

pub fn schema(c:&Connection)->rusqlite::Result<()> {
 c.execute_batch("CREATE TABLE IF NOT EXISTS app_projects(id INTEGER PRIMARY KEY, name TEXT NOT NULL, area TEXT NOT NULL CHECK(area IN ('personal','company')), description TEXT NOT NULL DEFAULT '', status TEXT NOT NULL DEFAULT 'active', version INTEGER NOT NULL DEFAULT 0, created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP);
 CREATE TABLE IF NOT EXISTS app_items(id INTEGER PRIMARY KEY, kind TEXT NOT NULL CHECK(kind IN ('task','note')), content TEXT NOT NULL, project_id INTEGER REFERENCES app_projects(id), status TEXT NOT NULL DEFAULT 'inbox', duration INTEGER, energy TEXT, version INTEGER NOT NULL DEFAULT 0, created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP);
 CREATE INDEX IF NOT EXISTS app_items_project_status ON app_items(project_id,status);
 CREATE TABLE IF NOT EXISTS app_commands(device_id TEXT NOT NULL, request_id TEXT NOT NULL, input TEXT NOT NULL, result TEXT NOT NULL, PRIMARY KEY(device_id,request_id));
 CREATE TABLE IF NOT EXISTS app_item_history(id INTEGER PRIMARY KEY, item_id INTEGER NOT NULL REFERENCES app_items(id), action TEXT NOT NULL, before_state TEXT NOT NULL, after_state TEXT NOT NULL, created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP);
 CREATE TABLE IF NOT EXISTS app_project_history(id INTEGER PRIMARY KEY, project_id INTEGER NOT NULL REFERENCES app_projects(id), before_state TEXT NOT NULL, after_state TEXT NOT NULL, created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP);")?;
 let has_original={let mut stmt=c.prepare("PRAGMA table_info(app_items)")?;let cols=stmt.query_map([],|r|r.get::<_,String>(1))?.collect::<Result<Vec<_>,_>>()?;cols.iter().any(|v|v=="original_content")};
 if !has_original {c.execute_batch("ALTER TABLE app_items ADD COLUMN original_content TEXT;")?;}
 c.execute("UPDATE app_items SET original_content=content WHERE original_content IS NULL",[])?;
 Ok(())
}
fn invalid(s:&str)->ApiError {ApiError(StatusCode::BAD_REQUEST,s.into())}
#[derive(Deserialize,Serialize)] pub struct NewProject{device_id:String,request_id:String,name:String,area:String,description:Option<String>}
pub async fn create_project(State(s):State<AppState>,Json(input):Json<NewProject>)->ApiResult<Value>{
 if input.device_id.is_empty()||input.device_id.len()>100||input.request_id.is_empty()||input.request_id.len()>100||input.name.trim().is_empty()||input.name.trim().chars().count()>200||input.description.as_deref().unwrap_or("").chars().count()>10000||!matches!(input.area.as_str(),"personal"|"company"){return Err(invalid("Invalid project"))}
 db_job(s,move|c|{
  let canonical=json!({"action":"create_project","data":input}).to_string();let tx=c.transaction()?;
  let previous:Option<(String,String)>=tx.query_row("SELECT input,result FROM app_commands WHERE device_id=?1 AND request_id=?2",params![input.device_id,input.request_id],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
  if let Some((old,result))=previous{
   if old!=canonical{return Err(ApiError(StatusCode::CONFLICT,"Request ID already used for different data".into()))}
   return serde_json::from_str(&result).map_err(|e|ApiError(StatusCode::INTERNAL_SERVER_ERROR,e.to_string()));
  }
  tx.execute("INSERT INTO app_projects(name,area,description) VALUES(?1,?2,?3)",params![input.name.trim(),input.area,input.description.unwrap_or_default()])?;
  let result=get_project(&tx,tx.last_insert_rowid())?;
  tx.execute("INSERT INTO app_commands(device_id,request_id,input,result) VALUES(?1,?2,?3,?4)",params![input.device_id,input.request_id,canonical,result.to_string()])?;
  tx.commit()?;Ok(result)
 }).await
}
fn get_project(c:&Connection,id:i64)->Result<Value,ApiError>{
 c.query_row("SELECT p.id,p.name,p.area,p.description,p.status,p.version,p.created_at,(SELECT count(*) FROM app_items i WHERE i.project_id=p.id AND i.kind='task' AND i.status NOT IN ('done','abandoned')) FROM app_projects p WHERE p.id=?1",[id],|r|Ok(json!({"id":r.get::<_,i64>(0)?,"name":r.get::<_,String>(1)?,"area":r.get::<_,String>(2)?,"description":r.get::<_,String>(3)?,"status":r.get::<_,String>(4)?,"version":r.get::<_,i64>(5)?,"created_at":r.get::<_,String>(6)?,"open_task_count":r.get::<_,i64>(7)?}))).optional()?.ok_or_else(||ApiError(StatusCode::NOT_FOUND,"Project not found".into()))
}
pub async fn projects(State(s):State<AppState>)->ApiResult<Vec<Value>>{db_job(s,|c|{
 let mut stmt=c.prepare("SELECT id FROM app_projects ORDER BY created_at DESC,id DESC")?;
 let ids=stmt.query_map([],|r|r.get::<_,i64>(0))?.collect::<Result<Vec<_>,_>>()?;
 ids.into_iter().map(|id|get_project(c,id)).collect()
}).await}
#[derive(Deserialize,Serialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateProject{device_id:String,request_id:String,expected_version:i64,name:String,description:String,status:String}
pub async fn update_project(State(s):State<AppState>,Path(id):Path<i64>,Json(input):Json<UpdateProject>)->ApiResult<Value>{
 if input.device_id.is_empty()||input.device_id.len()>100||input.request_id.is_empty()||input.request_id.len()>100||input.expected_version<0||input.name.trim().is_empty()||input.name.trim().chars().count()>200||input.description.chars().count()>10000||!matches!(input.status.as_str(),"active"|"paused"|"completed"|"archived"){return Err(invalid("Invalid project update"))}
 db_job(s,move|c|{
  let canonical=json!({"action":"update_project","project_id":id,"data":input}).to_string();
  let tx=c.transaction()?;
  let prior:Option<(String,String)>=tx.query_row("SELECT input,result FROM app_commands WHERE device_id=?1 AND request_id=?2",params![input.device_id,input.request_id],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
  if let Some((old,result))=prior {if old!=canonical{return Err(ApiError(StatusCode::CONFLICT,"Request ID already used for different data".into()))}return serde_json::from_str(&result).map_err(|e|ApiError(StatusCode::INTERNAL_SERVER_ERROR,e.to_string()))}
  let before=get_project(&tx,id)?;
  if before["version"].as_i64()!=Some(input.expected_version){return Err(ApiError(StatusCode::CONFLICT,"Project changed; refresh before retrying".into()))}
  if matches!(input.status.as_str(),"completed"|"archived") && before["open_task_count"].as_i64().unwrap_or(0)>0 {
   return Err(ApiError(StatusCode::CONFLICT,"Project has open tasks; complete, abandon, or move them before closing".into()))
  }
  if matches!(input.status.as_str(),"completed"|"archived") {
   let rules:i64=tx.query_row("SELECT count(*) FROM app_recurring_rules r JOIN app_items i ON i.id=r.template_task_id WHERE i.project_id=?1 AND r.status='active'",[id],|r|r.get(0))?;
   if rules>0{return Err(ApiError(StatusCode::CONFLICT,"End or pause recurring rules before closing the project".into()))}
  }
  tx.execute("UPDATE app_projects SET name=?1,description=?2,status=?3,version=version+1 WHERE id=?4 AND version=?5",params![input.name.trim(),input.description,input.status,id,input.expected_version])?;
  let after=get_project(&tx,id)?;
  tx.execute("INSERT INTO app_project_history(project_id,before_state,after_state) VALUES(?1,?2,?3)",params![id,before.to_string(),after.to_string()])?;
  tx.execute("INSERT INTO app_commands(device_id,request_id,input,result) VALUES(?1,?2,?3,?4)",params![input.device_id,input.request_id,canonical,after.to_string()])?;
  tx.commit()?;Ok(after)
 }).await
}
#[derive(Deserialize,Serialize)] pub struct Capture{device_id:String,request_id:String,kind:String,content:String,project_id:Option<i64>,duration:Option<i64>,energy:Option<String>}
pub async fn capture(State(s):State<AppState>,Json(input):Json<Capture>)->ApiResult<Value>{
 if input.device_id.is_empty()||input.device_id.len()>100||input.request_id.is_empty()||input.request_id.len()>100||!matches!(input.kind.as_str(),"task"|"note")||input.content.trim().is_empty()||input.content.chars().count()>20000||input.duration.is_some_and(|n|!(1..=1440).contains(&n))||input.energy.as_deref().is_some_and(|v|!matches!(v,"low"|"medium"|"high")){return Err(invalid("Invalid capture"))}
 db_job(s,move|c|{
  let canonical=serde_json::to_string(&input).map_err(|e|invalid(&e.to_string()))?;
  let tx=c.transaction()?;
  let previous:Option<(String,String)>=tx.query_row("SELECT input,result FROM app_commands WHERE device_id=?1 AND request_id=?2",params![input.device_id,input.request_id],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
  if let Some((old,result))=previous{
   if old!=canonical{return Err(ApiError(StatusCode::CONFLICT,"Request ID already used for different data".into()))}
   return serde_json::from_str(&result).map_err(|e|ApiError(StatusCode::INTERNAL_SERVER_ERROR,e.to_string()));
  }
  if let Some(id)=input.project_id{
   let valid:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM app_projects WHERE id=?1 AND status='active')",[id],|r|r.get(0))?;
   if !valid{return Err(invalid("Project does not exist or is not active"))}
  }
  let status=if input.project_id.is_some()&&input.kind=="task"{"ready"}else{"inbox"};
  tx.execute("INSERT INTO app_items(kind,content,original_content,project_id,status,duration,energy) VALUES(?1,?2,?2,?3,?4,?5,?6)",params![input.kind,input.content,input.project_id,status,input.duration,input.energy])?;
  let item=get_item(&tx,tx.last_insert_rowid())?;
  tx.execute("INSERT INTO app_commands(device_id,request_id,input,result) VALUES(?1,?2,?3,?4)",params![input.device_id,input.request_id,canonical,item.to_string()])?;
  tx.commit()?;Ok(item)
 }).await
}
pub fn get_item(c:&Connection,id:i64)->Result<Value,ApiError>{
 c.query_row("SELECT id,kind,content,project_id,status,duration,energy,version,created_at,updated_at,original_content,(SELECT rule_id FROM app_recurring_occurrences WHERE task_id=app_items.id),(SELECT occurrence_date FROM app_recurring_occurrences WHERE task_id=app_items.id) FROM app_items WHERE id=?1",[id],|r|Ok(json!({"id":r.get::<_,i64>(0)?,"kind":r.get::<_,String>(1)?,"content":r.get::<_,String>(2)?,"project_id":r.get::<_,Option<i64>>(3)?,"status":r.get::<_,String>(4)?,"duration":r.get::<_,Option<i64>>(5)?,"energy":r.get::<_,Option<String>>(6)?,"version":r.get::<_,i64>(7)?,"created_at":r.get::<_,String>(8)?,"updated_at":r.get::<_,String>(9)?,"original_content":r.get::<_,Option<String>>(10)?,"recurrence_rule_id":r.get::<_,Option<i64>>(11)?,"occurrence_date":r.get::<_,Option<String>>(12)?}))).optional()?.ok_or_else(||ApiError(StatusCode::NOT_FOUND,"Item not found".into()))
}
#[derive(Deserialize)] pub struct ItemQuery{project_id:Option<i64>,status:Option<String>}
pub async fn items(State(s):State<AppState>,Query(q):Query<ItemQuery>)->ApiResult<Vec<Value>>{db_job(s,move|c|{
 let mut stmt=c.prepare("SELECT id FROM app_items WHERE (?1 IS NULL OR project_id=?1) AND (?2 IS NULL OR status=?2) ORDER BY id DESC")?;
 let ids=stmt.query_map(params![q.project_id,q.status],|r|r.get::<_,i64>(0))?.collect::<Result<Vec<_>,_>>()?;
 ids.into_iter().map(|id|get_item(c,id)).collect()
}).await}

#[derive(Deserialize,Serialize)] pub struct Clarify {
 device_id:String, request_id:String, expected_version:i64, project_id:i64,
 duration:Option<i64>, energy:Option<String>,
}
pub async fn clarify(State(s):State<AppState>,Path(id):Path<i64>,Json(input):Json<Clarify>)->ApiResult<Value>{
 if input.device_id.is_empty()||input.device_id.len()>100||input.request_id.is_empty()||input.request_id.len()>100||input.expected_version<0||input.duration.is_some_and(|n|!(1..=1440).contains(&n))||input.energy.as_deref().is_some_and(|v|!matches!(v,"low"|"medium"|"high")){return Err(invalid("Invalid clarification"))}
 db_job(s,move|c|{
  // The action and item ID are part of the idempotency key's payload; a retry
  // cannot accidentally return a result from another command or another item.
  let canonical=json!({"action":"clarify","item_id":id,"data":input}).to_string();
  let tx=c.transaction()?;
  let previous:Option<(String,String)>=tx.query_row("SELECT input,result FROM app_commands WHERE device_id=?1 AND request_id=?2",params![input.device_id,input.request_id],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
  if let Some((old,result))=previous {
   if old!=canonical{return Err(ApiError(StatusCode::CONFLICT,"Request ID already used for different data".into()))}
   return serde_json::from_str(&result).map_err(|e|ApiError(StatusCode::INTERNAL_SERVER_ERROR,e.to_string()));
  }
  let before=get_item(&tx,id)?;
  let project_exists:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM app_projects WHERE id=?1 AND status='active')",[input.project_id],|r|r.get(0))?;
  if !project_exists{return Err(invalid("Project does not exist or is not active"))}
  let status=if before["kind"]=="task" {"ready"} else {"filed"};
  let rows=tx.execute("UPDATE app_items SET project_id=?1,duration=?2,energy=?3,status=?4,version=version+1,updated_at=CURRENT_TIMESTAMP WHERE id=?5 AND version=?6 AND status IN ('inbox','ready','filed')",params![input.project_id,input.duration,input.energy,status,id,input.expected_version])?;
  if rows==0{return Err(ApiError(StatusCode::CONFLICT,"Item changed; refresh before retrying".into()))}
  let after=get_item(&tx,id)?;
  tx.execute("INSERT INTO app_item_history(item_id,action,before_state,after_state) VALUES(?1,'clarify',?2,?3)",params![id,before.to_string(),after.to_string()])?;
  tx.execute("INSERT INTO app_commands(device_id,request_id,input,result) VALUES(?1,?2,?3,?4)",params![input.device_id,input.request_id,canonical,after.to_string()])?;
  tx.commit()?;Ok(after)
 }).await
}

#[derive(Deserialize,Serialize)] pub struct Transition {
 device_id:String, request_id:String, expected_version:i64, action:String, reason:Option<String>,
}
pub async fn transition(State(s):State<AppState>,Path(id):Path<i64>,Json(input):Json<Transition>)->ApiResult<Value>{
 if input.device_id.is_empty()||input.device_id.len()>100||input.request_id.is_empty()||input.request_id.len()>100||input.expected_version<0||!matches!(input.action.as_str(),"complete"|"abandon"|"undo") {return Err(invalid("Invalid action"))}
 let reason=input.reason.as_deref().unwrap_or("").trim();
 if (input.action=="abandon"&&reason.is_empty())||reason.chars().count()>2000{return Err(invalid("A reason is required to abandon a task"))}
 let out=db_job(s.clone(),move|c|{
  let canonical=json!({"action":"transition","item_id":id,"data":input}).to_string();
  let tx=c.transaction()?;
  let previous:Option<(String,String)>=tx.query_row("SELECT input,result FROM app_commands WHERE device_id=?1 AND request_id=?2",params![input.device_id,input.request_id],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
  if let Some((old,result))=previous {
   if old!=canonical{return Err(ApiError(StatusCode::CONFLICT,"Request ID already used for different data".into()))}
   return serde_json::from_str(&result).map_err(|e|ApiError(StatusCode::INTERNAL_SERVER_ERROR,e.to_string()));
  }
  let before=get_item(&tx,id)?;
  if before["kind"]!="task"{return Err(invalid("Only tasks have a completion status"))}
  if before["version"].as_i64()!=Some(input.expected_version){return Err(ApiError(StatusCode::CONFLICT,"Item changed; refresh before retrying".into()))}
  let current=before["status"].as_str().unwrap_or("");
  let next=match input.action.as_str(){
   "complete"|"abandon" if matches!(current,"inbox"|"ready")||(current=="decision"&&input.action=="abandon")=>if input.action=="complete"{"done".to_owned()}else{"abandoned".to_owned()},
   "undo" if matches!(current,"done"|"abandoned")=>{
    let last:Option<(String,String)>=tx.query_row("SELECT action,before_state FROM app_item_history WHERE item_id=?1 ORDER BY id DESC LIMIT 1",[id],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
    let (action,raw)=last.ok_or_else(||ApiError(StatusCode::CONFLICT,"No reversible action".into()))?;
    if !matches!(action.as_str(),"complete"|"abandon"){return Err(ApiError(StatusCode::CONFLICT,"Latest action is not reversible".into()))}
    let old:Value=serde_json::from_str(&raw).map_err(|e|ApiError(StatusCode::INTERNAL_SERVER_ERROR,e.to_string()))?;
    let restored=old["status"].as_str().filter(|s|matches!(*s,"inbox"|"ready"|"decision")).ok_or_else(||ApiError(StatusCode::CONFLICT,"Previous state cannot be restored".into()))?.to_owned();
    if let Some(project_id)=before["project_id"].as_i64(){
     let closed:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM app_projects WHERE id=?1 AND status IN ('completed','archived'))",[project_id],|r|r.get(0))?;
     if closed{return Err(ApiError(StatusCode::CONFLICT,"Reopen the project before restoring its task".into()))}
    }
    restored
   },
   _=>return Err(ApiError(StatusCode::CONFLICT,"Action does not apply to the current state".into())),
  };
  let count=tx.execute("UPDATE app_items SET status=?1,version=version+1,updated_at=CURRENT_TIMESTAMP WHERE id=?2 AND version=?3",params![next,id,input.expected_version])?;
  if count!=1{return Err(ApiError(StatusCode::CONFLICT,"Item changed; refresh before retrying".into()))}
  if input.action=="undo" {
   tx.execute("UPDATE app_task_reminders SET state=closed_from,closed_from=NULL,last_reset_alert=COALESCE((SELECT MAX(id) FROM app_reminder_alerts WHERE reminder_id=app_task_reminders.id),0),version=version+1 WHERE task_id=?1 AND state='closed' AND closed_from IS NOT NULL",[id])?;
  } else {
   tx.execute("UPDATE app_task_reminders SET closed_from=state,state='closed',version=version+1 WHERE task_id=?1 AND state IN ('scheduled','decision')",[id])?;
  }
  let after=get_item(&tx,id)?;
  let history=json!({"item":after,"reason":input.reason.as_deref().map(str::trim)});
  tx.execute("INSERT INTO app_item_history(item_id,action,before_state,after_state) VALUES(?1,?2,?3,?4)",params![id,input.action,before.to_string(),history.to_string()])?;
  tx.execute("INSERT INTO app_commands(device_id,request_id,input,result) VALUES(?1,?2,?3,?4)",params![input.device_id,input.request_id,canonical,after.to_string()])?;
  tx.commit()?;Ok(after)
 }).await?;
 s.revision.send_modify(|v|*v+=1);Ok(out)
}
pub async fn history(State(s):State<AppState>,Path(id):Path<i64>)->ApiResult<Vec<Value>>{db_job(s,move|c|{
 get_item(c,id)?;
 let mut stmt=c.prepare("SELECT id,action,before_state,after_state,created_at FROM app_item_history WHERE item_id=?1 ORDER BY id DESC")?;
 let rows=stmt.query_map([id],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?,r.get::<_,String>(4)?)))?.collect::<Result<Vec<_>,_>>()?;
 rows.into_iter().map(|(seq,action,before,after,date)|{
  let before:Value=serde_json::from_str(&before).map_err(|e|ApiError(StatusCode::INTERNAL_SERVER_ERROR,e.to_string()))?;
  let after:Value=serde_json::from_str(&after).map_err(|e|ApiError(StatusCode::INTERNAL_SERVER_ERROR,e.to_string()))?;
  Ok(json!({"id":seq,"action":action,"before":before,"after":after,"created_at":date}))
 }).collect()
}).await}

#[derive(Deserialize,Serialize)] pub struct EditContent {device_id:String,request_id:String,expected_version:i64,content:String}
pub async fn edit_content(State(s):State<AppState>,Path(id):Path<i64>,Json(input):Json<EditContent>)->ApiResult<Value>{
 if input.device_id.is_empty()||input.device_id.len()>100||input.request_id.is_empty()||input.request_id.len()>100||input.expected_version<0||input.content.trim().is_empty()||input.content.chars().count()>20000 {return Err(invalid("Invalid content"))}
 db_job(s,move|c|{
  let canonical=json!({"action":"edit_content","item_id":id,"data":input}).to_string();
  let tx=c.transaction()?;
  let previous:Option<(String,String)>=tx.query_row("SELECT input,result FROM app_commands WHERE device_id=?1 AND request_id=?2",params![input.device_id,input.request_id],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
  if let Some((old,result))=previous{
   if old!=canonical{return Err(ApiError(StatusCode::CONFLICT,"Request ID already used for different data".into()))}
   return serde_json::from_str(&result).map_err(|e|ApiError(StatusCode::INTERNAL_SERVER_ERROR,e.to_string()));
  }
  let before=get_item(&tx,id)?;
  if before["version"].as_i64()!=Some(input.expected_version){return Err(ApiError(StatusCode::CONFLICT,"Item changed; refresh before retrying".into()))}
  if !matches!(before["status"].as_str(),Some("inbox"|"ready"|"filed")){return Err(ApiError(StatusCode::CONFLICT,"Restore this item before editing".into()))}
  tx.execute("UPDATE app_items SET content=?1,version=version+1,updated_at=CURRENT_TIMESTAMP WHERE id=?2 AND version=?3",params![input.content,id,input.expected_version])?;
  let after=get_item(&tx,id)?;
  tx.execute("INSERT INTO app_item_history(item_id,action,before_state,after_state) VALUES(?1,'edit_content',?2,?3)",params![id,before.to_string(),after.to_string()])?;
  tx.execute("INSERT INTO app_commands(device_id,request_id,input,result) VALUES(?1,?2,?3,?4)",params![input.device_id,input.request_id,canonical,after.to_string()])?;
  tx.commit()?;Ok(after)
 }).await
}
