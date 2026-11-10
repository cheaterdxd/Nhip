/* No offline app/API cache: never serve stale task state or cache access keys. */
self.addEventListener('install',event=>event.waitUntil(self.skipWaiting()));
self.addEventListener('activate',event=>event.waitUntil(self.clients.claim()));
self.addEventListener('push',event=>{
 event.waitUntil((async()=>{
  const data=event.data?.json();if(!data||!Number.isSafeInteger(data.id)||!Number.isSafeInteger(data.task_id))return;
  await self.registration.showNotification('Nhịp · Cần phản hồi',{body:data.title||'Bạn có công việc cần xử lý',tag:`nhip-${data.id}`,data:{id:data.id,task_id:data.task_id},icon:'/icon-192.png'});
 })());
});
self.addEventListener('notificationclick',event=>{
 event.notification.close();const data=event.notification.data;
 event.waitUntil((async()=>{
  const url=new URL(`/?task_id=${encodeURIComponent(data.task_id)}&reminder_id=${encodeURIComponent(data.id)}`,self.location.origin).href;
  const windows=await self.clients.matchAll({type:'window',includeUncontrolled:true});
  for(const client of windows){if(new URL(client.url).origin===self.location.origin){await client.navigate(url);await client.focus();return;}}
  await self.clients.openWindow(url);
 })());
});
