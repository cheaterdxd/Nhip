// Exercise push and notification-click routing without claiming real iOS delivery.
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import vm from 'node:vm';

const handlers = {};
const shown = [], opened = [], focused = [], navigated = [];
let windows = [];
const self = {
  location: { origin: 'https://nhip.example.test' },
  addEventListener(name, fn) { handlers[name] = fn; },
  skipWaiting: async () => {},
  registration: { showNotification: async (title, options) => shown.push({title, options}) },
  clients: {
    claim: async () => {},
    matchAll: async () => windows,
    openWindow: async url => opened.push(url),
  },
};
vm.runInNewContext(await readFile(new URL('../public/sw.js', import.meta.url), 'utf8'), {self, URL, Number, encodeURIComponent});
async function fire(name, extra = {}) {
  let task;
  handlers[name]({ ...extra, waitUntil(promise) { task = promise; } });
  if (task) await task;
}
await fire('push', {data: {json: () => ({id: 17, task_id: 9501, title: 'Lời nhắc thử'})}});
assert.equal(shown.length, 1);
assert.equal(shown[0].options.tag, 'nhip-17');
assert.equal(shown[0].options.data.task_id, 9501);
await fire('push', {data: {json: () => ({id: 'bad', task_id: 9501})}});
assert.equal(shown.length, 1, 'invalid identifiers must not open a notification');
let closed = false;
const notification = {data: shown[0].options.data, close() { closed = true; }};
await fire('notificationclick', {notification});
assert.equal(closed, true);
assert.equal(opened[0], 'https://nhip.example.test/?task_id=9501&reminder_id=17');
const existing = {url: 'https://nhip.example.test/', navigate: async url => {navigated.push(url);return existing;}, focus: async () => focused.push(true)};
windows = [existing];
await fire('notificationclick', {notification});
assert.equal(navigated[0], opened[0]);
assert.equal(focused.length, 1);
assert.equal(opened.length, 1, 'existing client should be reused');
windows = [];
await fire('push', {data: {json: () => ({id: 1000000001, task_id: 42, source: 'app'})}});
const appNotification = {data: shown[1].options.data, close() {}};
await fire('notificationclick', {notification: appNotification});
assert.equal(opened[1], 'https://nhip.example.test/?task_id=42&reminder_id=1000000001&source=app');
await fire('push', {data: {json: () => ({id: 1500202645, task_id: 0, title: 'Báo cáo tuần đã sẵn sàng', source: 'report', week_start: '2026-11-02'})}});
assert.equal(shown[2].title, 'Nhịp · Báo cáo tuần');
const reportNotification = {data: shown[2].options.data, close() {}};
await fire('notificationclick', {notification: reportNotification});
assert.equal(opened[2], 'https://nhip.example.test/?report_week=2026-11-02');
console.log('PASS: push routing for task and weekly report, stable tags, invalid payload, client focus');
