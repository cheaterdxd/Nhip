/* No offline app/API cache: never serve stale task state or cache access keys. */
self.addEventListener('install',event=>event.waitUntil(self.skipWaiting()));
self.addEventListener('activate',event=>event.waitUntil(self.clients.claim()));
self.addEventListener('push',event=>{
 event.waitUntil((async()=>{
  let data;try{data=event.data?.json();}catch{return;}
  if(!data||!Number.isSafeInteger(data.id)||!Number.isSafeInteger(data.task_id))return;
  await self.registration.showNotification(data.source==='report'?'Nhịp · Báo cáo tuần':'Nhịp · Cần phản hồi',{body:data.title||'Bạn có công việc cần xử lý',tag:`nhip-${data.id}`,data:{id:data.id,task_id:data.task_id,source:['app','report'].includes(data.source)?data.source:'fixture',week_start:data.week_start||null},icon:'/icon-192.png'});
 })());
});
self.addEventListener('notificationclick',event=>{
 event.notification.close();const data=event.notification.data;
 event.waitUntil((async()=>{
  const path=data.source==='report'?`/?report_week=${encodeURIComponent(data.week_start)}`:`/?task_id=${encodeURIComponent(data.task_id)}&reminder_id=${encodeURIComponent(data.id)}${data.source==='app'?'&source=app':''}`;
  const url=new URL(path,self.location.origin).href;
  const windows=await self.clients.matchAll({type:'window',includeUncontrolled:true});
  for(const client of windows){if(new URL(client.url).origin===self.location.origin){try{const opened=await client.navigate(url);if(opened){await opened.focus();return;}}catch{/* Open a new app window below. */}}}
  await self.clients.openWindow(url);
 })());
});
