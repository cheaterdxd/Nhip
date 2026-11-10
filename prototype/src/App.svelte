<script lang="ts">
  import { onMount, tick } from 'svelte';
  import { invoke, isTauri } from '@tauri-apps/api/core';
  import { Sun, CalendarDays, Layers, Plus, Play, Check, X, Gauge, ArrowRight, RefreshCw } from 'lucide-svelte';
  import Focus from './Focus.svelte';
  import Reminders from './Reminders.svelte';
  import { request, time, frames, p95, type Task, type Project, type Slot } from './api';
  const fixtureDate = '2026-11-09';
  let page = $state<'today'|'calendar'|'metrics'>('today');
  let tasks = $state<Task[]>([]), projects = $state<Project[]>([]), events = $state<Task[]>([]);
  let focusId = $state<number|null>(null), timerRunning = $state(false), health = $state({ projects: 0, occurrences: 0 });
  let loading = $state(true), error = $state(''), notice = $state(''), capture = $state(false), saving = $state(false);
  let title = $state(''), notes = $state(''), duration = $state(25), projectId = $state(1), energy = $state('medium');
  let slots = $state<Slot[]>([]), slotReason = $state(''), finding = $state(false);
  let benchmarks = $state<Record<string, unknown>|null>(null), measuring = $state(false);
  let bootstrapMs = 0, uiReadyMs = 0;
  let selected = $derived(tasks.find(t=>t.id===focusId));
  const week = Array.from({length:7},(_,i)=>({ date:`2026-11-${String(9+i).padStart(2,'0')}`,name:['Thứ Hai','Thứ Ba','Thứ Tư','Thứ Năm','Thứ Sáu','Thứ Bảy','Chủ nhật'][i],day:9+i }));
  async function load() {
    const at=performance.now();
    [tasks,projects,events,health]=await Promise.all([
      request<Task[]>(`/api/tasks?date=${fixtureDate}&limit=100`), request<Project[]>('/api/projects'),
      request<Task[]>(`/api/calendar?start=${fixtureDate}`), request<typeof health>('/api/health')
    ]);
    focusId=tasks[0]?.id??null; bootstrapMs=performance.now()-at;
  }
  async function openReminder(id:number){try{const task=await request<Task>(`/api/tasks/${id}`);if(!tasks.some(t=>t.id===id))tasks=[task,...tasks];focusId=id;page='today';notice=`Công việc từ lời nhắc: ${task.title}`;}catch(e){error=String(e)}}
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
    (async()=>{
      try{
        await load();const linked=Number(new URLSearchParams(location.search).get('task_id'));if(linked>0)await openReminder(linked);loading=false;await tick();await frames();uiReadyMs=performance.now();
        document.body.dataset.ready=JSON.stringify({navigation_to_ready_ms:uiReadyMs,bootstrap_ms:bootstrapMs});
        if(isTauri()&&await invoke<boolean>('benchmark_enabled')){
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
  });
</script>
<div class="app">
  <aside class="sidebar">
    <div class="brand"><span><Check size={19}/></span>nhịp.</div>
    <button class="quick" onclick={()=>{capture=!capture;page='today'}}><Plus size={18}/>Ghi nhanh</button>
    <nav aria-label="Điều hướng">
      <button class:active={page==='today'} onclick={()=>page='today'}><Sun size={19}/>Hôm nay</button>
      <button class:active={page==='calendar'} onclick={()=>page='calendar'}><CalendarDays size={19}/>Lịch tuần</button>
      <button class:active={page==='metrics'} onclick={()=>page='metrics'}><Gauge size={19}/>Đo hiệu năng</button>
    </nav>
    <div class="fixture"><Layers size={17}/><span>{health.projects} dự án<br>{health.occurrences.toLocaleString('vi-VN')} lần thực hiện</span></div>
    <small class="scope">Prototype dữ liệu tổng hợp<br>Svelte + Tauri + Rust</small>
  </aside>
  <div class="workspace">
    <header><span>Không gian của tôi</span><span class="prototype-label">Prototype · dữ liệu thử</span></header>
    <main>
      <Reminders onOpen={openReminder}/>
      <div class="page-title"><div><h1>{page==='today'?'Hôm nay':page==='calendar'?'Lịch của tôi':'Đo hiệu năng'}</h1><p>Thứ Hai, 9 tháng 11 · mốc dữ liệu kiểm thử</p></div>{#if page==='today'}<button class="secondary" onclick={findSlots} disabled={!selected||finding}><CalendarDays size={16}/>{finding?'Đang tìm…':'Tìm giờ trống'}</button>{/if}</div>
      {#if error}<div class="error" role="alert">{error}<button class="text-action" onclick={()=>location.reload()}><RefreshCw size={15}/>Thử lại</button></div>{/if}
      {#if notice}<div class="notice" role="status">{notice}<button aria-label="Đóng thông báo" onclick={()=>notice=''}><X size={16}/></button></div>{/if}
      {#if loading}<p class="empty">Đang đọc dữ liệu từ backend Rust…</p>
      {:else if page==='today'}
        {#if capture}<form class="capture" onsubmit={e=>{e.preventDefault();createTask()}}>
          <div class="section-heading"><h2>Ghi nhanh công việc</h2><button type="button" class="icon" aria-label="Đóng, giữ bản nháp" onclick={()=>capture=false}><X size={18}/></button></div>
          <label for="task-title">Tên công việc</label><input id="task-title" placeholder="Bạn cần làm gì?" bind:value={title} maxlength="240"/>
          <div class="form-grid"><div><label for="project">Dự án</label><select id="project" bind:value={projectId}>{#each projects as p}<option value={p.id}>{p.name}</option>{/each}</select></div><div><label for="duration">Số phút dự kiến</label><input id="duration" type="number" min="1" max="1440" bind:value={duration}/></div><div><label for="energy">Mức tập trung</label><select id="energy" bind:value={energy}><option value="low">Nhẹ</option><option value="medium">Vừa</option><option value="high">Cao</option></select></div></div>
          <label for="notes">Ghi chú</label><textarea id="notes" bind:value={notes} placeholder="Thêm một chút bối cảnh…"></textarea>
          <div class="form-actions"><button class="primary" disabled={saving}>{saving?'Đang lưu…':'Lưu công việc'}</button></div>
        </form>{/if}
        {#if slotReason}<div class="slots"><h2>Giờ trống cho việc đang chọn</h2><p>{slotReason}</p>{#each slots as slot}<div>{slot.date} <strong>{time(slot.minute)} – {time(slot.minute+slot.duration)}</strong></div>{/each}<small>Prototype chỉ đề xuất, chưa ghi lịch hay thực thi đủ toàn bộ quy tắc sản phẩm.</small></div>{/if}
        <div class="today-layout"><section><div class="section-heading"><h2>Việc cần làm <span>{tasks.length}</span></h2><span class="muted">Bấm phát để chọn việc</span></div>
        {#each tasks as t (t.id)}<div class="task-row" class:selected={focusId===t.id}><button class="check" aria-label={'Hoàn thành '+t.title} onclick={()=>complete(t)}><span></span></button><div class="task-content"><strong>{t.title}</strong><span>{t.project_name} · {t.duration} phút · {time(t.minute)}</span></div><button class="task-play" aria-label={'Chọn '+t.title} onclick={()=>selectTask(t)}><Play size={17}/></button></div>{/each}
        {#if !tasks.length}<p class="empty">Không có công việc. Thêm một việc để thử.</p>{/if}<button class="text-action add" onclick={()=>capture=true}><Plus size={17}/>Thêm công việc</button></section>
        <Focus task={selected} onRunning={value=>timerRunning=value} onSaved={message=>notice=message}/></div>
      {:else if page==='calendar'}
        <div class="calendar-caption"><span>9–15 tháng 11</span><span>{events.length} phiên · tuần dữ liệu thử</span></div>
        <div class="week-grid">{#each week as day}<section class="day-column"><h2>{day.name}<span>{day.day}</span></h2>{#each events.filter(t=>t.date===day.date).slice(0,12) as event}<button class="calendar-event" onclick={()=>{selectTask(event);page='today'}}><small>{time(event.minute)}</small><span>{event.title}</span><small>{event.remaining} phút</small></button>{/each}{#if events.filter(t=>t.date===day.date).length>12}<p class="muted">Và {events.filter(t=>t.date===day.date).length-12} phiên khác</p>{/if}</section>{/each}</div>
      {:else}
        <div class="benchmark-panel"><h2>Đo trên giao diện đang chạy</h2><p>Đọc dữ liệu SQLite thật, chuyển màn hình và cập nhật task đã tải. Thời gian từ cập nhật state đến hai khung hình tiếp theo là phép đo tổng hợp, không phải INP từ người dùng thật.</p><button class="primary" onclick={measureUI} disabled={measuring||timerRunning}>{measuring?'Đang đo…':'Chạy 80 lượt đo giao diện'}<ArrowRight size={17}/></button>{#if benchmarks}<pre data-testid="benchmark-result">{JSON.stringify(benchmarks,null,2)}</pre>{/if}</div>
      {/if}
    </main>
  </div>
</div>
