export function formatTime(seconds: number, showMs = false): string {
  if (!isFinite(seconds) || seconds < 0) seconds = 0;
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  const s = Math.floor(seconds % 60);
  const ms = Math.floor((seconds % 1) * 1000);
  const pad = (n: number, l = 2) => String(n).padStart(l, "0");
  let out = h > 0 ? `${h}:${pad(m)}:${pad(s)}` : `${pad(m)}:${pad(s)}`;
  if (showMs) out += `.${pad(ms, 3)}`;
  return out;
}

export function uid(prefix = "id"): string {
  return `${prefix}_${Math.random().toString(36).slice(2, 10)}${Date.now().toString(36).slice(-4)}`;
}

export function clamp(v: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, v));
}

export function clamp01(v: number): number {
  return clamp(v, 0, 1);
}

let counter = 0;
export function stableId(prefix = "id"): string {
  counter += 1;
  return `${prefix}_${counter}_${Math.random().toString(36).slice(2, 6)}`;
}

export function download(text: string, filename: string, type = "application/json"): void {
  const blob = new Blob([text], { type });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = filename;
  a.click();
  URL.revokeObjectURL(url);
}

export function lighten(hex: string, percent: number): string {
  const num = parseInt(hex.replace("#", ""), 16);
  const r = Math.min(255, (num >> 16) + Math.round(255 * percent));
  const g = Math.min(255, ((num >> 8) & 0xff) + Math.round(255 * percent));
  const b = Math.min(255, (num & 0xff) + Math.round(255 * percent));
  return `#${((r << 16) | (g << 8) | b).toString(16).padStart(6, "0")}`;
}
