<script lang="ts">
  import { onDestroy } from 'svelte';
  import { Play, Pause, Timer, Square } from 'lucide-svelte';
  import { request, type Task } from './api';
  let { task, onRunning, onSaved }: { task: Task | undefined; onRunning: (value: boolean) => void; onSaved: (message: string) => void } = $props();
  let seconds = $state(1500), running = $state(false), elapsed = $state(0), saving = $state(false);
  let interval: ReturnType<typeof setInterval> | undefined;
  let last = 0;
  function stop() { clearInterval(interval); interval = undefined; running = false; onRunning(false); }
  function toggle() {
    if (running) { stop(); return; }
    if (!task) return;
    running = true; onRunning(true); last = Date.now();
    interval = setInterval(() => {
      const now = Date.now(), step = Math.floor((now-last)/1000);
      if (step < 1) return;
      last += step * 1000;
      elapsed += Math.min(seconds, step); seconds = Math.max(0,seconds-step);
      if (!seconds) { stop(); onSaved('Hết phiên. Bạn có thể lưu thời gian và nghỉ.'); }
    }, 1000);
  }
  async function finish() {
    if (!task || elapsed === 0) return;
    stop(); saving = true;
    try { await request('/api/sessions', { task_id: task.id, seconds: elapsed }); elapsed = 0; seconds = 1500; onSaved('Đã lưu thời gian vào SQLite.'); }
    catch (error) { onSaved(String(error)); } finally { saving = false; }
  }
  $effect(() => { const id = task?.id; if (id !== undefined) { stop(); seconds = 1500; elapsed = 0; } });
  onDestroy(() => clearInterval(interval));
</script>
<aside class="focus-panel">
  <div class="focus-heading"><Timer size={17}/><h2>Phiên tập trung</h2></div>
  <h3>{task?.title ?? 'Chọn một công việc'}</h3><p>{task?.project_name ?? 'Bấm nút phát ở một công việc.'}</p>
  <div class="timer-value" data-testid="pomodoro">{String(Math.floor(seconds/60)).padStart(2,'0')}:{String(seconds%60).padStart(2,'0')}</div>
  <p>{running ? 'Đang tập trung' : elapsed ? 'Đang tạm dừng' : '25 phút cho một việc'}</p>
  <button class="primary full" onclick={toggle} disabled={!task || saving}>{#if running}<Pause size={16}/>{:else}<Play size={16}/>{/if}{running?'Tạm dừng':elapsed?'Tiếp tục':'Bắt đầu'}</button>
  {#if elapsed > 0}<button class="text-action" onclick={finish} disabled={saving}><Square size={14}/>{saving?'Đang lưu…':'Kết thúc & lưu thời gian'}</button>{/if}
  <small>Nghỉ 5 phút sau mỗi phiên</small>
</aside>
