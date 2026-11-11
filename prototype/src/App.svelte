<script lang="ts">
  import { onMount, tick } from 'svelte';
  import { invoke, isTauri } from '@tauri-apps/api/core';
  import { Sun, CalendarDays, Layers, Plus, Play, Check, X, Gauge, ArrowRight, RefreshCw, ClipboardList } from 'lucide-svelte';
  import Focus from './Focus.svelte';
  import Reminders from './Reminders.svelte';
  import Product from './Product.svelte';
  import { request, frames, p95, type Task, type Project, type Slot } from './api';
  const fixtureDate = '2026-11-09';
  let page = $state<'today'|'calendar'|'metrics'|'product'|'report'>('today');
  let tasks = $state<Task[]>([]), projects = $state<Project[]>([]), events = $state<Task[]>([]);
  let focusId = $state<number|null>(null), timerRunning = $state(false), health = $state({ projects: 0, occurrences: 0 });
  let productTarget=$state<number|null>(null),productOpenCount=$state(0);
  let loading = $state(true), error = $state(''), notice = $state(''), capture = $state(false), saving = $state(false);
  let title = $state(''), notes = $state(''), duration = $state(25), projectId = $state(1), energy = $state('medium');
  let slots = $state<Slot[]>([]), slotReason = $state(''), finding = $state(false);
  let benchmarks = $state<Record<string, unknown>|null>(null), measuring = $state(false);
  type WeeklyReport={week_start:string;week_end:string;counts:{completed:number;postponements:number;alerts:number;overdue_now:number;focused_seconds:number};completed_tasks:{task_id:number;content:string;completed_at:number}[];postponements:{reminder_id:number;task_id:number;content:string;at:number;reason:string|null;due_at:number|null}[];observations:string[];ai_analysis_available:boolean};
  type CalendarAppointment={reminder_id:number;task_id:number;content:string;project:string;due_at:number;duration_minutes:number|null;state:string;snoozes:number;occurrence_date:string|null};
  type BusyEvent={id:number;title:string;starts_at:number;ends_at:number;version:number};
  type PlannedSession={id:number;task_id:number;content:string;starts_at:number;duration_minutes:number;state:string};
  type TodayTask={id:number;kind:'task'|'note';content:string;status:string;duration:number|null};
  type AppCalendar={start:string;end:string;appointments:CalendarAppointment[];overdue:CalendarAppointment[];focus_sessions:{id:number;task_id:number;content:string;started_at:number;ends_at:number;state:string}[];busy_events:BusyEvent[];planned_sessions:PlannedSession[]};
  let weeklyReport=$state<WeeklyReport|null>(null),reportLoading=$state(false);
  let todayCalendar=$state<AppCalendar|null>(null),todayOtherTasks=$state<TodayTask[]>([]),todayLoading=$state(false);
  let todayLoadSeq=0;
  const deviceId=localStorage.getItem('nhip-device')||crypto.randomUUID();localStorage.setItem('nhip-device',deviceId);
  let calendarMode=$state<'week'|'month'>('week'),calendarAnchor=$state(new Date()),appCalendar=$state<AppCalendar|null>(null),calendarLoading=$state(false);
  let busyForm=$state(false),busyEditing=$state<BusyEvent|null>(null),busyTitle=$state(''),busyStart=$state(''),busyEnd=$state(''),busySaving=$state(false);
  let calendarDates=$derived.by(()=>{
    const first=new Date(calendarAnchor.getFullYear(),calendarAnchor.getMonth(),calendarMode==='week'?calendarAnchor.getDate():1);
    if(calendarMode==='week'){first.setDate(first.getDate()-((first.getDay()+6)%7));return Array.from({length:7},(_,i)=>{const d=new Date(first);d.setDate(first.getDate()+i);return d;});}
    first.setDate(first.getDate()-((first.getDay()+6)%7));return Array.from({length:42},(_,i)=>{const d=new Date(first);d.setDate(first.getDate()+i);return d;});
  });
  let calendarHeading=$derived(calendarMode==='week'?calendarDates[0]?.toLocaleDateString('vi-VN',{day:'numeric',month:'long'})+' – '+calendarDates[6]?.toLocaleDateString('vi-VN',{day:'numeric',month:'long',year:'numeric'}):calendarAnchor.toLocaleDateString('vi-VN',{month:'long',year:'numeric'}));
  let bootstrapMs = 0, uiReadyMs = 0;
  let selected = $derived(tasks.find(t=>t.id===focusId));
  async function openQuickCapture(){page='product';await tick();document.getElementById('quick-content')?.focus();}
  async function loadToday(){const seq=++todayLoadSeq;todayLoading=true;try{const [calendar,items]=await Promise.all([request<AppCalendar>('/api/app/calendar?start='+isoDate(new Date())+'&days=1'),request<TodayTask[]>('/api/app/items')]);if(seq!==todayLoadSeq)return;const visible=new Set([...calendar.appointments,...calendar.overdue,...calendar.planned_sessions].map(row=>row.task_id));todayCalendar=calendar;todayOtherTasks=items.filter(item=>item.kind==='task'&&['inbox','ready','decision'].includes(item.status)&&!visible.has(item.id));error='';}catch(e){if(seq===todayLoadSeq)error=String(e);}finally{if(seq===todayLoadSeq)todayLoading=false;}}
  function openToday(){page='today';void loadToday();}
  async function load() {
    const at=performance.now();
    [tasks,projects,events,health]=await Promise.all([
      request<Task[]>(`/api/tasks?date=${fixtureDate}&limit=100`), request<Project[]>('/api/projects'),
      request<Task[]>(`/api/calendar?start=${fixtureDate}`), request<typeof health>('/api/health')
    ]);
    focusId=tasks[0]?.id??null; bootstrapMs=performance.now()-at;
  }
  async function openReminder(id:number,source:'app'|'fixture'|'report'='fixture',week?:string){if(source==='report'){await openWeeklyReport(week||weekStart());return;}if(source==='app'){productTarget=id;productOpenCount++;page='product';return;}try{const task=await request<Task>('/api/tasks/'+id);if(!tasks.some(t=>t.id===id))tasks=[task,...tasks];focusId=id;page='today';notice='Công việc từ lời nhắc: '+task.title;}catch(e){error=String(e)}}
  function selectTask(t:Task) {
    if(timerRunning&&focusId!==t.id){notice='Tạm dừng và lưu phiên hiện tại trước khi chuyển việc.';return;}
    focusId=t.id;notice='';
  }
  async function createTask() {
    if(!title.trim()){error='Nhập tên công việc trước khi lưu.';return;}
    saving=true;error='';
    try {
      const t=await request<Task>('/api/tasks',{project_id:projectId,title,notes,duration,energy});
      tasks=[t,...tasks];title='';notes='';capture=false;notice='Đã lưu công việc vào SQLite.';
      health.occurrences+=1;
    }catch(e){error=String(e)}finally{saving=false}
  }
  async function complete(t:Task) {
    try {await request(`/api/tasks/${t.id}/complete`,{expected_version:t.version},'PATCH'); tasks=tasks.filter(x=>x.id!==t.id);events=events.filter(x=>x.id!==t.id);if(focusId===t.id)focusId=tasks[0]?.id??null;notice='Đã hoàn thành.';}catch(e){error=String(e)}
  }
  async function findSlots() {
    if(!selected)return;finding=true;error='';
    try{const result=await request<{slots:Slot[];reason:string}>(`/api/slots?task_id=${selected.id}&start=${fixtureDate}`);slots=result.slots;slotReason=result.reason;}catch(e){error=String(e)}finally{finding=false}
  }
  function weekStart(){const date=new Date();date.setHours(0,0,0,0);date.setDate(date.getDate()-((date.getDay()+6)%7));return [date.getFullYear(),String(date.getMonth()+1).padStart(2,'0'),String(date.getDate()).padStart(2,'0')].join('-');}
  async function openWeeklyReport(targetWeek=weekStart()){page='report';reportLoading=true;error='';try{weeklyReport=await request<WeeklyReport>('/api/app/reports/weekly?week_start='+targetWeek);try{await request('/api/app/reports/weekly/'+targetWeek+'/ack',{device_id:deviceId});}catch{/* Báo cáo vẫn xem được khi chưa có bản ghi thông báo. */}}catch(e){error=String(e)}finally{reportLoading=false}}
  function minutes(seconds:number){return Math.floor(seconds/3600)+' giờ '+Math.floor(seconds%3600/60)+' phút';}
  function isoDate(date:Date){return [date.getFullYear(),String(date.getMonth()+1).padStart(2,'0'),String(date.getDate()).padStart(2,'0')].join('-');}
  function localIsoFromEpoch(seconds:number){return isoDate(new Date(seconds*1000));}
  async function loadAppCalendar(){calendarLoading=true;error='';const first=calendarDates[0],days=calendarMode==='week'?7:42;try{appCalendar=await request<AppCalendar>('/api/app/calendar?start='+isoDate(first)+'&days='+days);}catch(e){error=String(e)}finally{calendarLoading=false}}
  function openCalendar(){page='calendar';void loadAppCalendar();}
  function setCalendarMode(mode:'week'|'month'){calendarMode=mode;void loadAppCalendar();}
  function shiftCalendar(amount:number){const next=new Date(calendarAnchor);if(calendarMode==='week')next.setDate(next.getDate()+amount*7);else{next.setDate(1);next.setMonth(next.getMonth()+amount);}calendarAnchor=next;void loadAppCalendar();}
  function appointmentsOn(date:Date){const key=isoDate(date);return appCalendar?.appointments.filter(item=>localIsoFromEpoch(item.due_at)===key)||[];}
  function focusSessionsOn(date:Date){const key=isoDate(date);return appCalendar?.focus_sessions.filter(item=>localIsoFromEpoch(item.started_at)===key)||[];}
  function busyEventsOn(date:Date){const key=isoDate(date);return appCalendar?.busy_events.filter(item=>localIsoFromEpoch(item.starts_at)===key)||[];}
  function plannedSessionsOn(date:Date){const key=isoDate(date);return appCalendar?.planned_sessions.filter(item=>localIsoFromEpoch(item.starts_at)===key)||[];}
  function openBusyForm(){const date=isoDate(calendarAnchor);busyEditing=null;busyTitle='';busyStart=`${date}T18:00`;busyEnd=`${date}T19:00`;busyForm=true;}
  function editBusyEvent(event:BusyEvent){busyEditing=event;busyTitle=event.title;busyStart=new Date(event.starts_at*1000).toLocaleString('sv-SE').replace(' ','T').slice(0,16);busyEnd=new Date(event.ends_at*1000).toLocaleString('sv-SE').replace(' ','T').slice(0,16);busyForm=true;}
  async function createBusyEvent(){const start=Math.floor(new Date(busyStart).getTime()/1000),end=Math.floor(new Date(busyEnd).getTime()/1000);if(!busyTitle.trim()||!Number.isFinite(start)||!Number.isFinite(end)||end<=start){error='Nhập tên sự kiện và thời gian kết thúc sau thời gian bắt đầu.';return;}busySaving=true;error='';try{if(busyEditing){await request('/api/app/events/'+busyEditing.id,{expected_version:busyEditing.version,title:busyTitle,starts_at:start,ends_at:end},'PATCH');notice='Đã cập nhật giờ bận.';}else{await request('/api/app/events',{device_id:deviceId,request_id:crypto.randomUUID(),title:busyTitle,starts_at:start,ends_at:end});notice='Đã thêm giờ bận vào lịch.';}busyForm=false;busyEditing=null;busyTitle='';await loadAppCalendar();}catch(e){error=String(e)}finally{busySaving=false}}
  async function deleteBusyEvent(event:BusyEvent){try{await request('/api/app/events/'+event.id,undefined,'DELETE');notice='Đã xóa giờ bận.';await loadAppCalendar()}catch(e){error=String(e)}}
  async function measureUI() {
    if(measuring)return;measuring=true;error='';
    const original=page, switchTimes:number[]=[], selectTimes:number[]=[];
    // State-to-paint proxy; this is not a real-user INP measurement.
    for(let i=0;i<50;i++){const at=performance.now();page=i%2?'today':'calendar';await tick();await frames();switchTimes.push(performance.now()-at)}
    page='today';await tick();
    for(let i=0;i<30;i++){const at=performance.now();focusId=tasks[i%tasks.length]?.id??null;await tick();await frames();selectTimes.push(performance.now()-at)}
    page=original;
    const result={kind:'synthetic_state_to_two_animation_frames',bootstrap_ms:bootstrapMs,navigation_to_ready_ms:uiReadyMs,switch_screen_p95_ms:p95(switchTimes),select_task_p95_ms:p95(selectTimes),switch_screen_raw_ms:switchTimes,select_task_raw_ms:selectTimes,rendered_tasks:tasks.length,week_events:events.length,user_agent:navigator.userAgent,fixture:health};
    benchmarks=result;document.body.dataset.benchmark=JSON.stringify(result);measuring=false;return result;
  }
  onMount(()=>{
    const refreshVisible=()=>{if(!localStorage.getItem('nhip-key'))return;if(page==='today')void loadToday();else if(page==='calendar')void loadAppCalendar();else if(page==='report')void openWeeklyReport(weeklyReport?.week_start||weekStart());};
    const refreshOnReturn=()=>{if(!document.hidden)refreshVisible();};
    window.addEventListener('nhip-reminder-update',refreshVisible);
    document.addEventListener('visibilitychange',refreshOnReturn);
    (async()=>{
      try{
        const benchmarkEnabled=isTauri()&&await invoke<boolean>('benchmark_enabled');
        if(!benchmarkEnabled&&!localStorage.getItem('nhip-key')){error='Nhập mã truy cập trong phần Kết nối để tải dữ liệu của bạn.';loading=false;return;}
        const params=new URLSearchParams(location.search),linked=Number(params.get('task_id')),reportWeek=params.get('report_week'),source=params.get('source')==='app'?'app':'fixture';if(linked>0&&source==='app')await openReminder(linked,source);
        if(benchmarkEnabled)await load();
        await loadToday();if(linked>0&&source==='fixture')await openReminder(linked,source);if(reportWeek)await openReminder(0,'report',reportWeek);loading=false;await tick();await frames();uiReadyMs=performance.now();
        document.body.dataset.ready=JSON.stringify({navigation_to_ready_ms:uiReadyMs,bootstrap_ms:bootstrapMs});
        if(benchmarkEnabled){
          const mode=await invoke<string>('benchmark_mode');
          const result=mode==='interactions'?await measureUI():{navigation_to_ready_ms:uiReadyMs,bootstrap_ms:bootstrapMs,user_agent:navigator.userAgent,fixture:health};
          await invoke('finish_benchmark',{measurements:result});
        }
      }catch(e){
        error=String(e);loading=false;document.body.dataset.error=error;
        if(isTauri()){
          try{if(await invoke<boolean>('benchmark_enabled'))await invoke('finish_benchmark',{measurements:{error}})}catch{/* Keep the visible error if telemetry itself fails. */}
        }
      }
    })();
    return()=>{window.removeEventListener('nhip-reminder-update',refreshVisible);document.removeEventListener('visibilitychange',refreshOnReturn);};
  });
</script>
<div class="app">
  <aside class="sidebar">
    <div class="brand"><span><Check size={19}/></span>nhịp.</div>
    <button class="quick" onclick={openQuickCapture}><Plus size={18}/>Ghi nhanh</button>
    <nav aria-label="Điều hướng">
      <button class:active={page==='today'} onclick={openToday}><Sun size={19}/>Hôm nay</button>
      <button class:active={page==='product'} onclick={()=>page='product'}><Layers size={19}/>Dự án & Hộp thư</button>
      <button class:active={page==='calendar'} onclick={openCalendar}><CalendarDays size={19}/>Lịch tuần / tháng</button>
      <button class:active={page==='report'} onclick={()=>openWeeklyReport()}><ClipboardList size={19}/>Báo cáo tuần</button>
    </nav>
    <small class="scope">Prototype cá nhân<br>Svelte + Tauri + Rust</small>
  </aside>
  <div class="workspace">
    <header><span>Không gian của tôi</span><span class="prototype-label">Prototype · dữ liệu cục bộ</span></header>
    <main>
      <Reminders onOpen={openReminder}/>
      <div class="page-title"><div><h1>{page==='today'?'Hôm nay':page==='calendar'?'Lịch của tôi':page==='product'?'Công việc của tôi':page==='report'?'Báo cáo tuần':'Đo hiệu năng'}</h1><p>{page==='report'&&weeklyReport?weeklyReport.week_start+' – '+weeklyReport.week_end:new Date().toLocaleDateString('vi-VN',{weekday:'long',day:'numeric',month:'long',year:'numeric'})}</p></div>{#if page==='today'}<button class="secondary" onclick={loadToday}><RefreshCw size={16}/>Làm mới</button>{/if}</div>
      {#if error}<div class="error" role="alert">{error}<button class="text-action" onclick={()=>location.reload()}><RefreshCw size={15}/>Thử lại</button></div>{/if}
      {#if notice}<div class="notice" role="status">{notice}<button aria-label="Đóng thông báo" onclick={()=>notice=''}><X size={16}/></button></div>{/if}
      {#if loading}<p class="empty">Đang đọc dữ liệu từ backend Rust…</p>
      {:else if page==='today'}
        {#if todayLoading}<p class="empty">Đang tải công việc hôm nay…</p>{:else if todayCalendar}
          {#if todayCalendar.overdue.length}<section class="overdue-panel"><div class="section-heading"><h2>Cần xử lý · {todayCalendar.overdue.length}</h2></div>{#each todayCalendar.overdue as item(item.reminder_id)}<button class="overdue-row" onclick={()=>openReminder(item.task_id,'app')}><span>{item.content}</span><small>{item.state==='decision'?'Cần quyết định':'Quá hạn '+new Date(item.due_at*1000).toLocaleString('vi-VN')} · Hoãn {item.snoozes}/3</small></button>{/each}</section>{/if}
          <section><div class="section-heading"><h2>Lịch hôm nay</h2><button class="text-action" onclick={openCalendar}>Mở lịch</button></div>
            {#each todayCalendar.appointments.filter(item=>item.due_at>=Math.floor(Date.now()/1000)) as item(item.reminder_id)}<button class="task-row" onclick={()=>openReminder(item.task_id,'app')}><span class="task-content"><strong>{item.content}</strong><span>{new Date(item.due_at*1000).toLocaleTimeString('vi-VN',{hour:'2-digit',minute:'2-digit'})} · {item.project} · {item.duration_minutes??'—'} phút</span></span></button>{/each}
            {#each todayCalendar.planned_sessions as session(session.id)}<button class="task-row" onclick={()=>openReminder(session.task_id,'app')}><span class="task-content"><strong>{session.content}</strong><span>{new Date(session.starts_at*1000).toLocaleTimeString('vi-VN',{hour:'2-digit',minute:'2-digit'})} · Phiên {session.duration_minutes} phút · {session.state==='done'?'Đã hoàn tất':'Đã xếp lịch'}</span></span></button>{/each}
            {#each todayCalendar.busy_events as event(event.id)}<div class="task-row"><span class="task-content"><strong>{event.title}</strong><span>{new Date(event.starts_at*1000).toLocaleTimeString('vi-VN',{hour:'2-digit',minute:'2-digit'})} · Giờ bận</span></span></div>{/each}
            {#if !todayCalendar.appointments.some(item=>item.due_at>=Math.floor(Date.now()/1000))&&!todayCalendar.planned_sessions.length&&!todayCalendar.busy_events.length}<p class="empty">Chưa có việc nào sắp tới trong hôm nay.</p>{/if}
          </section>
          <section><div class="section-heading"><h2>Việc còn mở ngoài lịch hôm nay · {todayOtherTasks.length}</h2></div>
            {#each todayOtherTasks as item(item.id)}<button class="task-row" onclick={()=>openReminder(item.id,'app')}><span class="task-content"><strong>{item.content}</strong><span>{item.status==='decision'?'Cần quyết định':item.status==='inbox'?'Chưa phân loại':'Đang thực hiện'}{item.duration?` · ${item.duration} phút`:''}</span></span></button>{/each}
            {#if !todayOtherTasks.length}<p class="muted">Không có task nào khác đang mở.</p>{/if}
          </section>
          <button class="text-action add" onclick={()=>page='product'}><Layers size={17}/>Xem mọi công việc và ghi chú</button>
        {/if}
      {:else if page==='product'}
        <Product targetId={productTarget} openCount={productOpenCount}/>
      {:else if page==='calendar'}
        <div class="calendar-toolbar">
          <div class="calendar-stepper"><button class="secondary" onclick={()=>shiftCalendar(-1)} aria-label="Khoảng trước">‹</button><strong>{calendarHeading}</strong><button class="secondary" onclick={()=>shiftCalendar(1)} aria-label="Khoảng sau">›</button><button class="text-action" onclick={()=>{calendarAnchor=new Date();void loadAppCalendar()}}>Hôm nay</button></div>
          <div class="calendar-switch"><button class:active={calendarMode==='week'} onclick={()=>setCalendarMode('week')}>Tuần</button><button class:active={calendarMode==='month'} onclick={()=>setCalendarMode('month')}>Tháng</button><button class="secondary" onclick={openBusyForm}>Thêm giờ bận</button></div>
        </div>
        {#if busyForm}<form class="capture" onsubmit={e=>{e.preventDefault();void createBusyEvent()}}><label for="busy-title">Tên việc bận</label><input id="busy-title" bind:value={busyTitle} maxlength="200" required/><label for="busy-start">Bắt đầu</label><input id="busy-start" type="datetime-local" bind:value={busyStart} required/><label for="busy-end">Kết thúc</label><input id="busy-end" type="datetime-local" bind:value={busyEnd} required/><div class="form-actions"><button type="button" class="secondary" onclick={()=>{busyForm=false;busyEditing=null}}>Hủy</button><button class="primary" disabled={busySaving}>{busySaving?'Đang lưu…':busyEditing?'Lưu thay đổi':'Lưu giờ bận'}</button></div></form>{/if}
        {#if calendarLoading}<p class="empty">Đang tải lịch…</p>{:else}
          {#if appCalendar?.overdue.length}<section class="overdue-panel"><h2>Quá hạn · {appCalendar.overdue.length}</h2>{#each appCalendar.overdue as item(item.task_id)}<button class="overdue-row" onclick={()=>openReminder(item.task_id,'app')}><span>{item.content}</span><small>{item.state==='decision'?'Cần quyết định':'Hẹn '+new Date(item.due_at*1000).toLocaleString('vi-VN')} · Hoãn {item.snoozes}/3</small></button>{/each}</section>{/if}
          {#if calendarMode==='week'}
            <div class="week-grid app-calendar-week">{#each calendarDates as day}<section class="day-column"><h2>{['Chủ nhật','Thứ Hai','Thứ Ba','Thứ Tư','Thứ Năm','Thứ Sáu','Thứ Bảy'][day.getDay()]}<span>{day.getDate()}</span></h2>{#each busyEventsOn(day) as event(event.id)}<div class="calendar-focus"><small>{new Date(event.starts_at*1000).toLocaleTimeString('vi-VN',{hour:'2-digit',minute:'2-digit'})}–{new Date(event.ends_at*1000).toLocaleTimeString('vi-VN',{hour:'2-digit',minute:'2-digit'})}</small><span>Bận · {event.title}</span><button class="text-action" onclick={()=>editBusyEvent(event)}>Sửa</button><button class="text-action" onclick={()=>deleteBusyEvent(event)}>Xóa</button></div>{/each}{#each plannedSessionsOn(day) as session(session.id)}<div class="calendar-event"><small>{new Date(session.starts_at*1000).toLocaleTimeString('vi-VN',{hour:'2-digit',minute:'2-digit'})} · {session.duration_minutes} phút</small><span>{session.content}</span><small>{session.state==='done'?'Đã hoàn tất':'Phiên đã xếp'}</small></div>{/each}{#each appointmentsOn(day) as item(item.task_id)}<button class="calendar-event" class:decision={item.state==='decision'} onclick={()=>openReminder(item.task_id,'app')}><small>{new Date(item.due_at*1000).toLocaleTimeString('vi-VN',{hour:'2-digit',minute:'2-digit'})}</small><span>{item.content}</span><small>{item.duration_minutes||'—'} phút · {item.project}</small></button>{/each}{#each focusSessionsOn(day) as session(session.id)}<div class="calendar-focus"><small>{new Date(session.started_at*1000).toLocaleTimeString('vi-VN',{hour:'2-digit',minute:'2-digit'})}</small><span>Tập trung · {session.content}</span></div>{/each}{#if !busyEventsOn(day).length&&!plannedSessionsOn(day).length&&!appointmentsOn(day).length&&!focusSessionsOn(day).length}<p class="muted calendar-empty-day">Trống</p>{/if}</section>{/each}</div>
          {:else}
            <div class="month-weekdays">{#each ['Thứ Hai','Thứ Ba','Thứ Tư','Thứ Năm','Thứ Sáu','Thứ Bảy','Chủ nhật'] as label}<span>{label}</span>{/each}</div>
            <div class="month-grid">{#each calendarDates as day}<section class:outside-month={day.getMonth()!==calendarAnchor.getMonth()} class="month-day"><span class="month-day-number">{day.getDate()}</span>{#each busyEventsOn(day).slice(0,2) as event(event.id)}<div class="month-event" title={event.title}><time>{new Date(event.starts_at*1000).toLocaleTimeString('vi-VN',{hour:'2-digit',minute:'2-digit'})}</time><span>Bận · {event.title}</span></div>{/each}{#each plannedSessionsOn(day).slice(0,2) as session(session.id)}<div class="month-event" title={session.content}><time>{new Date(session.starts_at*1000).toLocaleTimeString('vi-VN',{hour:'2-digit',minute:'2-digit'})}</time><span>{session.content}</span></div>{/each}{#each appointmentsOn(day).slice(0,3) as item(item.task_id)}<button class="month-event" class:decision={item.state==='decision'} title={item.content} onclick={()=>openReminder(item.task_id,'app')}><time>{new Date(item.due_at*1000).toLocaleTimeString('vi-VN',{hour:'2-digit',minute:'2-digit'})}</time><span>{item.content}</span></button>{/each}{#if appointmentsOn(day).length>3}<small class="muted">+{appointmentsOn(day).length-3} task</small>{/if}</section>{/each}</div>
          {/if}
        {/if}
      {:else if page==='report'}
        {#if reportLoading}<p class="empty">Đang tổng hợp dữ liệu tuần…</p>{:else if weeklyReport}
          <section class="weekly-report">
            <div class="report-metrics">
              <article><span>Đã hoàn thành</span><strong>{weeklyReport.counts.completed}</strong></article>
              <article><span>Task quá hạn hiện tại</span><strong>{weeklyReport.counts.overdue_now}</strong></article>
              <article><span>Lượt nhắc</span><strong>{weeklyReport.counts.alerts}</strong></article>
              <article><span>Thời gian tập trung</span><strong>{minutes(weeklyReport.counts.focused_seconds)}</strong></article>
            </div>
            <section class="report-section"><h2>Lý do hoãn · {weeklyReport.counts.postponements}</h2>{#if weeklyReport.postponements.length}{#each weeklyReport.postponements as item}<article class="report-row"><strong>{item.content}</strong><span>{item.reason||'Không ghi lý do'}{item.due_at?' · Hẹn lại '+new Date(item.due_at*1000).toLocaleString('vi-VN'):''}</span></article>{/each}{:else}<p class="muted">Tuần này chưa ghi nhận lượt hoãn.</p>{/if}</section>
            <section class="report-section"><h2>Việc đã hoàn thành · {weeklyReport.completed_tasks.length}</h2>{#if weeklyReport.completed_tasks.length}{#each weeklyReport.completed_tasks as item}<article class="report-row"><strong>{item.content}</strong><span>{new Date(item.completed_at*1000).toLocaleString('vi-VN')}</span></article>{/each}{:else}<p class="muted">Chưa có task hoàn thành trong tuần này.</p>{/if}</section>
            <section class="report-section"><h2>Quan sát từ dữ liệu</h2>{#each weeklyReport.observations as item}<p>{item}</p>{/each}<small>Các nhận xét được tổng hợp từ số liệu ứng dụng.</small></section>
            <section class="report-section ai-report-section">
              <h2>Gợi ý cải thiện</h2>
              <p class="muted">Phân tích AI sẽ được bổ sung sau. Hiện báo cáo hiển thị số liệu, việc đã hoàn thành và lý do hoãn.</p>
            </section>
          </section>
        {/if}
      {:else}
        <div class="benchmark-panel"><h2>Đo trên giao diện đang chạy</h2><p>Đọc dữ liệu SQLite thật, chuyển màn hình và cập nhật task đã tải. Thời gian từ cập nhật state đến hai khung hình tiếp theo là phép đo tổng hợp, không phải INP từ người dùng thật.</p><button class="primary" onclick={measureUI} disabled={measuring||timerRunning}>{measuring?'Đang đo…':'Chạy 80 lượt đo giao diện'}<ArrowRight size={17}/></button>{#if benchmarks}<pre data-testid="benchmark-result">{JSON.stringify(benchmarks,null,2)}</pre>{/if}</div>
      {/if}
    </main>
  </div>
</div>
