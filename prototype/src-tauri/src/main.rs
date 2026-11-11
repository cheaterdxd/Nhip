#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::{Manager, Emitter};
use tauri_plugin_notification::NotificationExt;
use serde_json::{json, Value};
use std::{path::PathBuf, time::Instant};

struct BenchmarkState {
    started: Instant,
    output: Option<PathBuf>,
}

#[tauri::command]
fn benchmark_enabled(state: tauri::State<'_, BenchmarkState>) -> bool {
    state.output.is_some()
}

#[tauri::command]
fn benchmark_mode() -> String {
    std::env::var("NHIP_BENCH_MODE").unwrap_or_else(|_| "startup".into())
}

#[tauri::command]
fn finish_benchmark(
    app: tauri::AppHandle,
    state: tauri::State<'_, BenchmarkState>,
    measurements: Value,
) -> Result<(), String> {
    let Some(path) = &state.output else {
        return Err("Benchmark mode is not enabled".into());
    };
    let report = json!({
        "process_start_to_result_ms": state.started.elapsed().as_secs_f64() * 1000.0,
        "mode": benchmark_mode(),
        "client": measurements,
        "build": if cfg!(debug_assertions) { "debug" } else { "release" }
    });
    let data = serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?;
    let temp = path.with_extension("partial");
    std::fs::write(&temp, data).map_err(|e| e.to_string())?;
    std::fs::rename(temp, path).map_err(|e| e.to_string())?;
    if let Some(w)=app.get_webview_window("main") {
        match std::env::var("NHIP_PROFILE_STATE").as_deref(){Ok("minimized")=>{let _=w.minimize();},Ok("hidden")=>{let _=w.hide();},_=>{}}
    }
    if std::env::var("NHIP_BENCH_STAY_OPEN").as_deref() != Ok("1") {
        app.exit(0);
    }
    Ok(())
}

#[tauri::command]
fn notify_reminder(app: tauri::AppHandle, id:i32, task_id:i64, title:String, source:String, week_start:Option<String>)->Result<(),String>{
 let builder=app.notification().builder().id(id).title("Nhịp · Cần phản hồi").body(title).extra("task_id",task_id).extra("reminder_id",id).extra("source",source);
 let builder=if let Some(week)=week_start{builder.extra("week_start",week)}else{builder};
 builder.show().map_err(|e|e.to_string())
}
#[tauri::command]
fn dismiss_reminder(app:tauri::AppHandle,id:i32)->Result<(),String>{app.notification().remove_active(vec![id]).map_err(|e|e.to_string())}
fn main() {
    let state = BenchmarkState {
        started: Instant::now(),
        output: std::env::var_os("NHIP_BENCH_OUTPUT").map(PathBuf::from),
    };
    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            let handle=app.handle().clone();
            app.notification().on_action(move |action| {
                if action.action_id()=="dismiss" {return;}
                if let Some(n)=action.notification() {
                    if let Some(w)=handle.get_webview_window("main") {let _=w.unminimize();let _=w.show();let _=w.set_focus();}
                    let _=handle.emit("reminder-open",n.extra());
                }
            })?;
            if std::env::var_os("NHIP_BENCH_OUTPUT").is_none() {
                use tauri::{menu::{Menu,MenuItem},tray::TrayIconBuilder};
                let show=MenuItem::with_id(app,"show","Mở Nhịp",true,None::<&str>)?;
                let quit=MenuItem::with_id(app,"quit","Thoát hoàn toàn",true,None::<&str>)?;
                let menu=Menu::with_items(app,&[&show,&quit])?;
                let mut tray=TrayIconBuilder::new().tooltip("Nhịp · đang chạy nền").menu(&menu).on_menu_event(|app,event| {
                    if event.id.as_ref()=="quit" {app.exit(0)}
                    if event.id.as_ref()=="show" {if let Some(w)=app.get_webview_window("main"){let _=w.show();let _=w.unminimize();let _=w.set_focus();}}
                });
                if let Some(icon)=app.default_window_icon(){tray=tray.icon(icon.clone());}
                tray.build(app)?;
            }
            if std::env::var_os("NHIP_PROFILE_MINIMAL").is_some(){
                if let Some(w)=app.get_webview_window("main"){w.navigate(w.url()?.join("/minimal.html")?)?;}
            }
            Ok(())
        })
        .on_window_event(|window,event| {
            if let tauri::WindowEvent::CloseRequested{api,..}=event {
                if std::env::var_os("NHIP_BENCH_OUTPUT").is_none(){api.prevent_close();let _=window.hide();}
            }
        })
        .manage(state)
        .invoke_handler(tauri::generate_handler![benchmark_enabled, benchmark_mode, finish_benchmark, notify_reminder, dismiss_reminder])
        .run(tauri::generate_context!())
        .expect("Cannot start Nhịp desktop");
}
