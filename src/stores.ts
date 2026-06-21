import { writable, derived, get } from "svelte/store";
import type { Clip, MediaAsset, Project, Track, TrackType } from "./types";
import { stableId, clamp } from "./lib/util";

export const DEFAULT_PROJECT: Project = {
  name: "Untitled",
  width: 1920,
  height: 1080,
  fps: 30,
  tracks: [],
  assets: {},
};

export function createEmptyProject(): Project {
  const videoTrack: Track = {
    id: stableId("track"),
    type: "video",
    name: "V1",
    clips: [],
    muted: false,
    hidden: false,
    locked: false,
  };
  const audioTrack: Track = {
    id: stableId("track"),
    type: "audio",
    name: "A1",
    clips: [],
    muted: false,
    hidden: false,
    locked: false,
  };
  return {
    ...structuredClone(DEFAULT_PROJECT),
    name: "Untitled",
    tracks: [videoTrack, audioTrack],
  };
}

export function defaultClip(asset: MediaAsset): Clip {
  const dur = asset.duration > 0 ? asset.duration : 5;
  return {
    id: stableId("clip"),
    mediaId: asset.id,
    name: asset.name,
    sourceStart: 0,
    sourceEnd: dur,
    timelineStart: 0,
    volume: 1,
    filters: { brightness: 0, contrast: 0, saturation: 0 },
    textOverlay: null,
    fadeIn: 0,
    fadeOut: 0,
    isImage: asset.type === "image",
  };
}

// ---------- Project store with undo/redo ----------
const HISTORY_LIMIT = 60;

class HistoryStore {
  private past: Project[] = [];
  private future: Project[] = [];
  readonly store = writable<Project>(createEmptyProject());
  dirty = writable(false);
  private suppress = false;

  get = (): Project => get(this.store);

  set = (p: Project, record = true): void => {
    if (record && !this.suppress) {
      this.pushHistory();
    }
    this.store.set(p);
    if (record) this.dirty.set(true);
  };

  update = (fn: (p: Project) => Project, record = true): void => {
    if (record && !this.suppress) {
      this.pushHistory();
    }
    this.store.update(fn);
    if (record) this.dirty.set(true);
  };

  /** Apply mutation without recording history (e.g. during dragging). */
  updateSilent = (fn: (p: Project) => Project): void => {
    this.store.update(fn);
  };

  commit = (): void => {
    // Call after a series of silent updates to push to history + mark dirty.
    this.pushHistory();
    this.dirty.set(true);
    // re-trigger subscribers
    this.store.update((p) => ({ ...p }));
  };

  private pushHistory = (): void => {
    this.past.push(structuredClone(get(this.store)));
    if (this.past.length > HISTORY_LIMIT) this.past.shift();
    this.future = [];
  };

  undo = (): void => {
    if (this.past.length === 0) return;
    const current = structuredClone(get(this.store));
    const prev = this.past.pop()!;
    this.future.push(current);
    this.suppress = true;
    this.store.set(prev);
    this.suppress = false;
    this.dirty.set(true);
  };

  redo = (): void => {
    if (this.future.length === 0) return;
    const current = structuredClone(get(this.store));
    const next = this.future.pop()!;
    this.past.push(current);
    this.suppress = true;
    this.store.set(next);
    this.suppress = false;
    this.dirty.set(true);
  };

  canUndo = derived(this.store, () => this.past.length > 0);
  // Note: derived won't react to internal arrays; expose as plain getters
  getUndoState = () => ({ canUndo: this.past.length > 0, canRedo: this.future.length > 0 });

  markSaved = (): void => this.dirty.set(false);

  reset = (p: Project): void => {
    this.past = [];
    this.future = [];
    this.store.set(p);
    this.dirty.set(false);
  };
}

export const projectStore = new HistoryStore();
export const project = projectStore.store;
export const dirty = projectStore.dirty;

export const canUndoStore = writable(false);
export const canRedoStore = writable(false);

// Refresh undo/redo availability after each mutation
function refreshHistoryFlags(): void {
  const s = projectStore.getUndoState();
  canUndoStore.set(s.canUndo);
  canRedoStore.set(s.canRedo);
}
project.subscribe(refreshHistoryFlags);

export function undo(): void {
  projectStore.undo();
  refreshHistoryFlags();
}
export function redo(): void {
  projectStore.redo();
  refreshHistoryFlags();
}
export function commitHistory(): void {
  projectStore.commit();
  refreshHistoryFlags();
}

// ---------- Playback / UI stores ----------
export const currentTime = writable(0);
export const isPlaying = writable(false);
export const duration = derived(project, ($p) => projectDuration($p));
export const selectedClipId = writable<string | null>(null);
export const zoom = writable(80); // pixels per second
export const theme = writable<"dark" | "light">(
  ((): "dark" | "light" => {
    try {
      const saved = localStorage.getItem("kve.theme");
      if (saved === "light" || saved === "dark") return saved;
    } catch {
      /* ignore */
    }
    return "dark";
  })(),
);

theme.subscribe((val) => {
  try {
    localStorage.setItem("kve.theme", val);
    document.documentElement.setAttribute("data-theme", val);
  } catch {
    /* ignore */
  }
});

// ---------- Derived helpers ----------
export function projectDuration(p: Project): number {
  let max = 0;
  for (const track of p.tracks) {
    for (const clip of track.clips) {
      const end = clip.timelineStart + (clip.sourceEnd - clip.sourceStart);
      if (end > max) max = end;
    }
  }
  return max;
}

export function findClip(p: Project, clipId: string): { track: Track; clip: Clip } | null {
  for (const track of p.tracks) {
    const clip = track.clips.find((c) => c.id === clipId);
    if (clip) return { track, clip };
  }
  return null;
}

export function addMediaToProject(asset: MediaAsset): void {
  projectStore.update((p) => {
    p.assets[asset.id] = asset;
    return { ...p };
  });
}

export function removeMedia(assetId: string): void {
  projectStore.update((p) => {
    delete p.assets[assetId];
    for (const track of p.tracks) {
      track.clips = track.clips.filter((c) => c.mediaId !== assetId);
    }
    return { ...p };
  });
}

export function addTrack(type: TrackType): void {
  projectStore.update((p) => {
    const count = p.tracks.filter((t) => t.type === type).length + 1;
    const prefix = type === "video" ? "V" : "A";
    p.tracks.push({
      id: stableId("track"),
      type,
      name: `${prefix}${count}`,
      clips: [],
      muted: false,
      hidden: false,
      locked: false,
    });
    return { ...p };
  });
}

export function deleteTrack(trackId: string): void {
  projectStore.update((p) => {
    p.tracks = p.tracks.filter((t) => t.id !== trackId);
    return { ...p };
  });
}

/** Find the next free position (end of last clip) on a track. */
export function nextFreeStart(track: Track): number {
  if (track.clips.length === 0) return 0;
  let max = 0;
  for (const c of track.clips) {
    const end = c.timelineStart + (c.sourceEnd - c.sourceStart);
    if (end > max) max = end;
  }
  return max;
}

/** Add a clip to a track at an optional position. */
export function addClipToTrack(
  trackId: string,
  asset: MediaAsset,
  start?: number,
): void {
  projectStore.update((p) => {
    const track = p.tracks.find((t) => t.id === trackId);
    if (!track) return p;
    const clip = defaultClip(asset);
    // For images, default to 5s
    if (asset.type === "image") {
      clip.sourceEnd = 5;
    }
    clip.timelineStart = start ?? nextFreeStart(track);
    // snap to avoid overlap if possible
    track.clips.push(clip);
    track.clips.sort((a, b) => a.timelineStart - b.timelineStart);
    return { ...p };
  });
}

export function moveClip(
  clipId: string,
  toTrackId: string,
  newStart: number,
): void {
  projectStore.update((p) => {
    let clip: Clip | undefined;
    for (const track of p.tracks) {
      const idx = track.clips.findIndex((c) => c.id === clipId);
      if (idx >= 0) {
        clip = track.clips.splice(idx, 1)[0];
        break;
      }
    }
    if (!clip) return p;
    const dest = p.tracks.find((t) => t.id === toTrackId);
    if (!dest) return p;
    clip.timelineStart = Math.max(0, newStart);
    dest.clips.push(clip);
    dest.clips.sort((a, b) => a.timelineStart - b.timelineStart);
    return { ...p };
  });
}

export function updateClip(clipId: string, patch: Partial<Clip>): void {
  projectStore.update((p) => {
    for (const track of p.tracks) {
      const idx = track.clips.findIndex((c) => c.id === clipId);
      if (idx >= 0) {
        track.clips[idx] = { ...track.clips[idx], ...patch };
        break;
      }
    }
    return { ...p };
  });
}

export function trimClip(
  clipId: string,
  side: "start" | "end",
  value: number,
): void {
  projectStore.update((p) => {
    for (const track of p.tracks) {
      const clip = track.clips.find((c) => c.id === clipId);
      if (!clip) continue;
      if (side === "start") {
        clip.sourceStart = clamp(value, 0, clip.sourceEnd - 0.05);
      } else {
        clip.sourceEnd = clamp(value, clip.sourceStart + 0.05, Infinity);
      }
      break;
    }
    return { ...p };
  });
}

export function splitAt(time: number): void {
  projectStore.update((p) => {
    for (const track of p.tracks) {
      if (track.locked) continue;
      const toSplit: Clip[] = [];
      for (const clip of track.clips) {
        const dur = clip.sourceEnd - clip.sourceStart;
        const start = clip.timelineStart;
        const end = start + dur;
        if (time > start + 0.05 && time < end - 0.05) {
          toSplit.push(clip);
        }
      }
      for (const clip of toSplit) {
        const offset = time - clip.timelineStart;
        const left: Clip = structuredClone(clip);
        left.id = stableId("clip");
        left.sourceEnd = clip.sourceStart + offset;
        const right: Clip = structuredClone(clip);
        right.id = stableId("clip");
        right.sourceStart = clip.sourceStart + offset;
        right.timelineStart = time;
        const idx = track.clips.indexOf(clip);
        track.clips.splice(idx, 1, left, right);
      }
    }
    return { ...p };
  });
}

export function deleteClip(clipId: string): void {
  projectStore.update((p) => {
    for (const track of p.tracks) {
      if (track.locked) continue;
      const before = track.clips.length;
      track.clips = track.clips.filter((c) => c.id !== clipId);
      if (track.clips.length !== before) break;
    }
    return { ...p };
  });
  if (get(selectedClipId) === clipId) selectedClipId.set(null);
}

export function duplicateClip(clipId: string): void {
  const p = get(project);
  for (const track of p.tracks) {
    const clip = track.clips.find((c) => c.id === clipId);
    if (clip) {
      const copy = structuredClone(clip);
      copy.id = stableId("clip");
      copy.timelineStart = clip.timelineStart + (clip.sourceEnd - clip.sourceStart);
      updateClipTrack(track.id, [...track.clips, copy]);
      break;
    }
  }
}

function updateClipTrack(trackId: string, clips: Clip[]): void {
  projectStore.update((p) => {
    const t = p.tracks.find((x) => x.id === trackId);
    if (t) {
      t.clips = clips;
      t.clips.sort((a, b) => a.timelineStart - b.timelineStart);
    }
    return { ...p };
  });
}

export function setProjectName(name: string): void {
  projectStore.update((p) => ({ ...p, name }));
}

export function setCanvasResolution(width: number, height: number): void {
  projectStore.update((p) => ({ ...p, width, height }));
}

export function getActiveClipsAt(p: Project, time: number): Clip[] {
  const result: Clip[] = [];
  for (const track of p.tracks) {
    if (track.type !== "video" || track.hidden) continue;
    for (const clip of track.clips) {
      const dur = clip.sourceEnd - clip.sourceStart;
      if (time >= clip.timelineStart && time < clip.timelineStart + dur) {
        result.push(clip);
        break;
      }
    }
  }
  return result;
}
