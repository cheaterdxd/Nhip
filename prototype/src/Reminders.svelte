<script lang="ts">
 import {onMount} from 'svelte';
 import {isTauri,invoke} from '@tauri-apps/api/core';
 import {listen} from '@tauri-apps/api/event';
 import {API,request} from './api';
 let {onOpen}:{onOpen:(id:number)=>void}=$props();
 type Reminder={id:number;task_id:number;title:string;ack_device:string|null};
 let pending=$state<Reminder[]>([]), message=$state(''), server=$state(API), key=$state(localStorage.getItem('nhip-key')||''), enabled=$state(false);
 const device=localStorage.getItem('nhip-device')||crypto.randomUUID();localStorage.setItem('nhip-device',device);
 let stop=false, revision=-1, running=false;
 let seen: number[]=JSON.parse(localStorage.getItem('nhip-seen')||'[]');
 let unlisten:(()=>void)|undefined;
 function settings(){try{const url=new URL(server);if(url.protocol!=='https:'&&!['127.0.0.1','localhost'].includes(url.hostname))throw Error('Dùng HTTPS cho server từ xa');localStorage.setItem('nhip-server',url.origin);localStorage.setItem('nhip-key',key);location.reload();}catch(e){message=String(e)}}
 async function poll(){
  if(running)return;running=true;
  while(!stop){try{
   const response=await fetch(`${API}/api/devices/${device}/updates?since=${revision}`,{headers:localStorage.getItem('nhip-key')?{authorization:`Bearer ${localStorage.getItem('nhip-key')}`}:{},signal:AbortSignal.timeout(35000)});
   if(!response.ok)throw Error(`Kết nối nhắc việc: HTTP ${response.status}`);
   const data=await response.json();revision=data.revision;pending=data.reminders.filter((r:Reminder)=>!r.ack_device);
   if(isTauri())for(const r of data.reminders as Reminder[]){
    if(r.ack_device){if(seen.includes(r.id))await invoke('dismiss_reminder',{id:r.id});continue;}
    if(!seen.includes(r.id)){await invoke('notify_reminder',{id:r.id,taskId:r.task_id,title:r.title});seen=[...seen,r.id].slice(-500);localStorage.setItem('nhip-seen',JSON.stringify(seen));}
   }
   message='Đã kết nối kênh nhắc việc.';
  }catch(e){message=String(e);await new Promise(resolve=>setTimeout(resolve,5000));}}
  running=false;
 }
 async function enable(){try{await request('/api/devices',{id:device,channel:isTauri()?'desktop':'browser'});localStorage.setItem('nhip-reminders','1');enabled=true;void poll();}catch(e){message=String(e)}}
 async function push(){try{
  if(!('serviceWorker'in navigator)||!('PushManager'in window))throw Error('Cần mở PWA trên màn hình chính iPhone qua HTTPS.');
  const permission=await Notification.requestPermission();if(permission!=='granted')throw Error('Chưa cấp quyền thông báo.');
  const reg=await navigator.serviceWorker.register('/sw.js');await navigator.serviceWorker.ready;
  const config=await request<{public_key:string|null}>('/api/push/config');if(!config.public_key)throw Error('Backend chưa cấu hình khóa VAPID.');
  const raw=atob(config.public_key.replace(/-/g,'+').replace(/_/g,'/'));const publicKey=Uint8Array.from(raw,c=>c.charCodeAt(0));
  const subscription=await reg.pushManager.getSubscription()||await reg.pushManager.subscribe({userVisibleOnly:true,applicationServerKey:publicKey});
  await request('/api/devices',{id:device,channel:'webpush',subscription:subscription.toJSON()});localStorage.setItem('nhip-push','1');localStorage.setItem('nhip-reminders','1');enabled=true;void poll();message='Đã đăng ký Web Push.';
 }catch(e){message=String(e)}}
 async function test(){try{const tasks=await request<{id:number}[]>('/api/tasks?limit=1');if(!tasks.length)throw Error('Chưa có task để nhắc');const result=await request<{id:number}>('/api/reminders',{request_id:crypto.randomUUID(),task_id:tasks[0].id,delay_seconds:10});message=`Lời nhắc #${result.id} sẽ tới sau 10 giây. Có thể đóng cửa sổ vào khay.`;}catch(e){message=String(e)}}
 async function ack(id:number){try{await request(`/api/reminders/${id}/ack`,{device_id:device});pending=pending.filter(r=>r.id!==id);message='Đã ghi nhận phản hồi chung cho các thiết bị.';}catch(e){message=String(e)}}
 onMount(()=>{
  if(isTauri())void listen<{task_id:number}>('reminder-open',event=>onOpen(event.payload.task_id)).then(fn=>{unlisten=fn});
  if(localStorage.getItem('nhip-reminders')==='1'){enabled=true;void poll();}
  return()=>{stop=true;unlisten?.()};
 });
</script>
<details class="capture"><summary>Kết nối & thử nhắc việc</summary>
 <label for="backend-url">Server backend</label><input id="backend-url" bind:value={server} placeholder="https://pc.ten-tailnet.ts.net"/>
 <label for="access-key">Mã truy cập</label><input id="access-key" type="password" bind:value={key}/>
 <button class="secondary" onclick={settings}>Lưu kết nối</button>
 <button class="secondary" onclick={enable}>Bật kênh nhắc trên máy này</button>
 {#if !isTauri()}<button class="secondary" onclick={push}>Đăng ký thông báo Web Push</button>{/if}
 <button class="secondary" onclick={test} disabled={!enabled}>Nhắc thử sau 10 giây</button>
 <p role="status">{message}</p><small>Mã thiết bị: {device}. Thử nghiệm nền tảng; dữ liệu tổng hợp.</small>
</details>
{#each pending as r(r.id)}<div class="notice"><span>#{r.id} · {r.title}</span><button onclick={()=>onOpen(r.task_id)}>Mở công việc</button><button onclick={()=>ack(r.id)}>Đã phản hồi</button></div>{/each}
