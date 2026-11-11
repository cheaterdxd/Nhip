<script lang="ts">
 import {onMount} from 'svelte';
 import {isTauri,invoke} from '@tauri-apps/api/core';
 import {listen} from '@tauri-apps/api/event';
 import {API,request} from './api';
 let {onOpen}:{onOpen:(id:number,source:'app'|'fixture'|'report',weekStart?:string)=>void}=$props();
 type Reminder={id:number;task_id:number;title:string;ack_device:string|null;source:'app'|'fixture'|'report';week_start?:string;reminder_id?:number;due_at?:number;version?:number;state?:'scheduled'|'decision'|'closed';kind?:'reminder'|'decision'|'weekly_report'};
 type AppItem={id:number;kind:'task'|'note';content:string;status:string;version:number};
 type AppSchedule={id:number;task_id:number;state:string;version:number};
 let pending=$state<Reminder[]>([]), message=$state(''), server=$state(API), key=$state(localStorage.getItem('nhip-key')||''), enabled=$state(false), pushEnabled=$state(localStorage.getItem('nhip-push')==='1');
 const needsSetup=!localStorage.getItem('nhip-key');
 let postponing=$state<number|null>(null),postponeAt=$state(''),postponeReason=$state(''),savingPostpone=$state(false);
 const device=localStorage.getItem('nhip-device')||crypto.randomUUID();localStorage.setItem('nhip-device',device);
 let stop=false, revision=-1, running=false;
 // Reminder IDs are only unique within one backend; keep delivery history per server.
 const seenKey=`nhip-seen:${API}`;
 let seen: number[]=JSON.parse(localStorage.getItem(seenKey)||'[]');
 let unlisten:(()=>void)|undefined;
 function isTailnetIPv4(hostname:string){const parts=hostname.split('.').map(Number);return parts.length===4&&parts.every(n=>Number.isInteger(n)&&n>=0&&n<=255)&&parts[0]===100&&parts[1]>=64&&parts[1]<=127;}
 function settings(){try{const url=new URL(server),local=['127.0.0.1','localhost','::1'].includes(url.hostname),tailnet=isTauri()&&isTailnetIPv4(url.hostname);if(url.protocol!=='https:'&&!local&&!tailnet)throw Error('Trình duyệt/PWA cần HTTPS khi kết nối server từ xa. App desktop Tauri có thể dùng HTTP qua IP Tailscale (100.64–100.127.x.x).');if(url.origin!==API){localStorage.removeItem('nhip-reminders');localStorage.removeItem('nhip-push');localStorage.removeItem('nhip-device');}localStorage.setItem('nhip-server',url.origin);localStorage.setItem('nhip-key',key);location.reload();}catch(e){message=String(e)}}
 async function poll(){
  if(running)return;running=true;
  while(!stop){try{
   const query=revision<0?'':`?since=${revision}`;
   const response=await fetch(`${API}/api/devices/${device}/updates${query}`,{headers:localStorage.getItem('nhip-key')?{authorization:`Bearer ${localStorage.getItem('nhip-key')}`}:{},signal:AbortSignal.timeout(35000)});
   if(!response.ok)throw Error(`Kết nối nhắc việc: HTTP ${response.status}`);
   const data=await response.json();const changedRevision=revision!==data.revision;revision=data.revision;pending=data.reminders.filter((r:Reminder)=>!r.ack_device);
   if(changedRevision)window.dispatchEvent(new Event('nhip-reminder-update'));
   if(isTauri()){
    let changed=false;
    for(const r of data.reminders as Reminder[]){
     if(r.ack_device){if(seen.includes(r.id)){await invoke('dismiss_reminder',{id:r.id});seen=seen.filter(id=>id!==r.id);changed=true;}continue;}
     if(!seen.includes(r.id)){await invoke('notify_reminder',{id:r.id,taskId:r.task_id,title:r.title,source:r.source||'fixture',weekStart:r.week_start||null});seen=[...seen,r.id];changed=true;}
    }
    if(changed)localStorage.setItem(seenKey,JSON.stringify(seen));
   }
   message=pushEnabled?'Web Push đã đăng ký; dữ liệu đang đồng bộ.':'Đang đồng bộ khi app mở. Bật Web Push để nhận khi app đóng.';
  }catch(e){message=String(e);await new Promise(resolve=>setTimeout(resolve,5000));}}
  running=false;
 }
 async function enable(){try{if(!isTauri()&&localStorage.getItem('nhip-push')==='1'){await push();return;}await request('/api/devices',{id:device,channel:isTauri()?'desktop':'browser'});localStorage.setItem('nhip-reminders','1');enabled=true;void poll();}catch(e){message=String(e)}}
 async function push(){try{
  if(!('serviceWorker'in navigator)||!('PushManager'in window))throw Error('Cần mở PWA trên màn hình chính iPhone qua HTTPS.');
  const permission=await Notification.requestPermission();if(permission!=='granted')throw Error('Chưa cấp quyền thông báo.');
  const reg=await navigator.serviceWorker.register('/sw.js');await navigator.serviceWorker.ready;
  const config=await request<{public_key:string|null}>('/api/push/config');if(!config.public_key)throw Error('Backend chưa cấu hình khóa VAPID.');
  const encoded=config.public_key.replace(/-/g,'+').replace(/_/g,'/');const raw=atob(encoded.padEnd(Math.ceil(encoded.length/4)*4,'='));const publicKey=Uint8Array.from(raw,c=>c.charCodeAt(0));
  let subscription=await reg.pushManager.getSubscription();
  const previous=subscription?.options.applicationServerKey;
  if(subscription&&(!previous||new Uint8Array(previous).length!==publicKey.length||!new Uint8Array(previous).every((byte,index)=>byte===publicKey[index]))){await subscription.unsubscribe();subscription=null;}
  subscription=subscription||await reg.pushManager.subscribe({userVisibleOnly:true,applicationServerKey:publicKey});
  await request('/api/devices',{id:device,channel:'webpush',subscription:subscription.toJSON()});localStorage.setItem('nhip-push','1');localStorage.setItem('nhip-reminders','1');pushEnabled=true;enabled=true;void poll();message='Đã đăng ký Web Push. App có thể nhận thông báo khi đóng.';
 }catch(e){message=String(e)}}
 async function test(){try{
  const [items,schedules]=await Promise.all([request<AppItem[]>('/api/app/items'),request<AppSchedule[]>('/api/app/reminders')]);
  const scheduled=new Set(schedules.map(row=>row.task_id));
  const task=items.find(item=>item.kind==='task'&&['inbox','ready'].includes(item.status)&&!scheduled.has(item.id));if(!task)throw Error('Không có task chưa đặt lịch nhắc. Tạo task thử mới để tránh đổi lịch công việc đang có.');
  const due=Math.floor(Date.now()/1000)+10;
  await request('/api/app/reminders',{device_id:device,request_id:crypto.randomUUID(),task_id:task.id,expected_task_version:task.version,due_at:due});
  message=`Đã đặt lời nhắc trên task “${task.content}” sau 10 giây. Kiểm tra thông báo, mở đúng task, rồi thử hoãn.`;
 }catch(e){message=String(e)}}
 async function ack(id:number){try{await request(`/api/reminders/${id}/ack`,{device_id:device});pending=pending.filter(r=>r.id!==id);message='Đã ghi nhận phản hồi chung cho các thiết bị.';}catch(e){message=String(e)}}
 function startPostpone(r:Reminder){postponing=r.id;postponeAt=localInput(Math.floor(Date.now()/1000)+900);postponeReason='';}
 function localInput(seconds:number){const date=new Date(seconds*1000),pad=(n:number)=>String(n).padStart(2,'0');return `${date.getFullYear()}-${pad(date.getMonth()+1)}-${pad(date.getDate())}T${pad(date.getHours())}:${pad(date.getMinutes())}`;}
 async function postpone(r:Reminder){
  if(r.source!=='app'||!r.reminder_id||r.version===undefined)return;
  const due=Math.floor(new Date(postponeAt).getTime()/1000);if(!Number.isFinite(due)||due<=Math.floor(Date.now()/1000)){message='Chọn thời điểm trong tương lai.';return;}
  if(!postponeReason.trim()){message='Nhập lý do hoãn để ghi vào lịch sử task.';return;}
  savingPostpone=true;try{await request(`/api/app/reminders/${r.reminder_id}/reschedule`,{device_id:device,request_id:crypto.randomUUID(),expected_version:r.version,due_at:due,reason:postponeReason.trim()});pending=pending.filter(x=>x.id!==r.id);postponing=null;message='Đã ghi nhận việc hoãn và lý do trên server. Các thiết bị sẽ nhận trạng thái mới.';window.dispatchEvent(new Event('nhip-reminder-update'));}
  catch(e){message=String(e)}finally{savingPostpone=false;}
 }
 onMount(()=>{
  if(isTauri())void listen<{task_id:number;source?:string;week_start?:string}>('reminder-open',event=>onOpen(event.payload.task_id,event.payload.source==='app'?'app':event.payload.source==='report'?'report':'fixture',event.payload.week_start)).then(fn=>{unlisten=fn});
  if(localStorage.getItem('nhip-reminders')==='1'){enabled=true;void poll();}
  return()=>{stop=true;unlisten?.()};
 });
</script>
<details class="capture" open={needsSetup}><summary>Kết nối & thử nhắc việc</summary>
 {#if needsSetup}<p class="muted">Nhập mã truy cập của PC chủ rồi lưu kết nối để bắt đầu.</p>{/if}
 <label for="backend-url">Server backend</label><input id="backend-url" bind:value={server} placeholder="https://pc.ten-tailnet.ts.net"/>
 <label for="access-key">Mã truy cập</label><input id="access-key" type="password" bind:value={key}/>
 <button class="secondary" onclick={settings}>Lưu kết nối</button>
 <button class="secondary" onclick={enable}>{isTauri()?'Bật thông báo trên máy này':'Đồng bộ lời nhắc khi app mở'}</button>
 {#if !isTauri()}<button class="primary" onclick={push}>{pushEnabled?'Cập nhật đăng ký Web Push':'Bật thông báo khi app đóng (Web Push)'}</button>{/if}
 <button class="secondary" onclick={test} disabled={!enabled}>Nhắc thử sau 10 giây</button>
 <p role="status">{message}</p><small>{!isTauri()?(pushEnabled?'Web Push đã được bật trên thiết bị này.':'Chưa bật Web Push trên thiết bị này.'):''} Mã thiết bị: {device}.</small>
</details>
{#each pending as r(r.id)}<div class="notice"><span>{r.kind==='decision'?'Cần chọn phương án · ':''}{r.title}</span>{#if r.source==='report'}<button onclick={()=>onOpen(0,'report',r.week_start)}>Xem báo cáo</button>{:else}<button onclick={()=>onOpen(r.task_id,r.source||'fixture')}>Mở công việc</button>{/if}{#if r.source==='app'&&r.state==='scheduled'}<button onclick={()=>startPostpone(r)}>Hoãn</button>{:else if r.source==='fixture'}<button onclick={()=>ack(r.id)}>Đã phản hồi</button>{/if}
 {#if postponing===r.id}<form class="capture" onsubmit={e=>{e.preventDefault();void postpone(r)}}><label for={'postpone-at-'+r.id}>Nhắc lại vào</label><input id={'postpone-at-'+r.id} type="datetime-local" bind:value={postponeAt} required/><label for={'postpone-reason-'+r.id}>Lý do hoãn</label><textarea id={'postpone-reason-'+r.id} bind:value={postponeReason} maxlength="2000" required></textarea><button class="primary" disabled={savingPostpone}>{savingPostpone?'Đang lưu…':'Lưu lịch mới'}</button></form>{/if}
</div>{/each}
