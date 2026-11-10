export type Project = { id: number; name: string; area: 'company' | 'personal' };
export type Task = {
  id: number; project_id: number; project_name: string; area: string;
  title: string; notes: string; duration: number; remaining: number;
  date: string | null; minute: number | null; status: string; energy: string; version: number;
};
export type Slot = { date: string; minute: number; duration: number };
export const API = localStorage.getItem('nhip-server') || import.meta.env.VITE_API_BASE || (location.protocol === 'https:' ? location.origin : 'http://127.0.0.1:4876');
export async function request<T>(path: string, body?: unknown, method?: string): Promise<T> {
  const response = await fetch(API + path, {
    method: method || (body ? 'POST' : 'GET'),
    headers: { ...(body ? { 'content-type': 'application/json' } : {}), ...(localStorage.getItem('nhip-key') ? {authorization: `Bearer ${localStorage.getItem('nhip-key')}`} : {}) },
    body: body ? JSON.stringify(body) : undefined,
    signal: AbortSignal.timeout(10000),
  });
  if (!response.ok) throw new Error((await response.json()).error || `HTTP ${response.status}`);
  return response.json();
}
export function time(minute: number | null) {
  return minute === null ? 'Chưa xếp lịch' : `${String(Math.floor(minute / 60)).padStart(2, '0')}:${String(minute % 60).padStart(2, '0')}`;
}
export const frames = () => new Promise<void>(resolve => requestAnimationFrame(() => requestAnimationFrame(() => resolve())));
export const p95 = (values: number[]) => [...values].sort((a,b) => a-b)[Math.ceil(values.length * .95) - 1] ?? 0;
