<script lang="ts">
  import {
    project,
    currentTime,
    duration,
    isPlaying,
    selectedClipId,
    getActiveClipsAt,
  } from "../stores";
  import { assetUrl } from "../lib/api";
  import { formatTime } from "../lib/util";
  import { t } from "../i18n";
  import { get } from "svelte/store";
  import Icon from "./Icon.svelte";
  import type { Clip, MediaAsset } from "../types";

  // Map of asset path -> resolved object URL for use in <video>/<img>
  const urlCache = new Map<string, string>();

  async function getUrl(asset: MediaAsset): Promise<string> {
    const key = asset.id;
    if (urlCache.has(key)) return urlCache.get(key)!;
    const url = await assetUrl(asset.path);
    urlCache.set(key, url);
    return url;
  }

  // resolved url per media id (reactive via reassignment)
  let resolved: Record<string, string> = {};
  let resolving = new Set<string>();

  async function ensureResolved(assetId: string): Promise<void> {
    if (resolved[assetId] || resolving.has(assetId)) return;
    const asset = $project.assets[assetId];
    if (!asset) return;
    resolving.add(assetId);
    try {
      const url = await getUrl(asset);
      resolved = { ...resolved, [assetId]: url };
    } finally {
      resolving.delete(assetId);
    }
  }

  // Whenever project assets change, ensure resolutions
  $: Object.keys($project.assets).forEach(ensureResolved);

  // ----- Playback clock -----
  let raf = 0;
  let lastTs = 0;

  function loop(ts: number): void {
    if (!get(isPlaying)) {
      raf = 0;
      return;
    }
    if (!lastTs) lastTs = ts;
    const dt = (ts - lastTs) / 1000;
    lastTs = ts;
    let next = get(currentTime) + dt;
    const dur = get(duration);
    if (next >= dur) {
      next = dur;
      isPlaying.set(false);
      currentTime.set(dur);
    } else {
      currentTime.set(next);
    }
    // Detect clip transitions and activate the new clip's media promptly.
    manageActiveMedia(next);
    syncMedia(next);
    if (get(isPlaying)) raf = requestAnimationFrame(loop);
  }

  // Previous set of active clip ids — used to detect transitions.
  let prevActiveIds: Set<string> = new Set();

  function manageActiveMedia(time: number): void {
    const active = activeClipIds(time);
    // Start playing newly-active clips
    for (const id of active) {
      if (!prevActiveIds.has(id)) {
        const vEl = videoEls[id];
        if (vEl && vEl.paused && get(isPlaying)) vEl.play().catch(() => {});
        const aEl = audioEls[id];
        if (aEl && aEl.paused && get(isPlaying)) aEl.play().catch(() => {});
      }
    }
    // Pause clips that are no longer active
    for (const id of prevActiveIds) {
      if (!active.has(id)) {
        const vEl = videoEls[id];
        if (vEl && !vEl.paused) vEl.pause();
        const aEl = audioEls[id];
        if (aEl && !aEl.paused) aEl.pause();
      }
    }
    prevActiveIds = active;
  }

  function play(): void {
    const dur = get(duration);
    if (get(currentTime) >= dur - 0.01) currentTime.set(0);
    isPlaying.set(true);
    lastTs = 0;
    cancelAnimationFrame(raf);
    raf = requestAnimationFrame(loop);
    playAllMedia();
  }

  function pause(): void {
    isPlaying.set(false);
    cancelAnimationFrame(raf);
    pauseAllMedia();
  }

  function togglePlay(): void {
    if (get(isPlaying)) pause();
    else play();
  }

  function stop(): void {
    pause();
    currentTime.set(0);
  }

  function stepFrame(dir: 1 | -1): void {
    pause();
    const fps = $project.fps || 30;
    currentTime.update((t) => Math.max(0, t + dir * (1 / fps)));
    syncMedia(get(currentTime));
  }

  function skip(dir: 1 | -1): void {
    pause();
    currentTime.update((t) => Math.max(0, t + dir * 5));
    syncMedia(get(currentTime));
  }

  isPlaying.subscribe((playing) => {
    if (!playing) {
      cancelAnimationFrame(raf);
    }
  });

  // ----- Media element management -----
  // We render <video>/<img> for each visible clip but only ACTIVATE the ones
  // near the playhead (current + next). Keeping all videos decoding at once
  // causes stuttering; pausing off-screen ones frees the decoder.
  let videoEls: Record<string, HTMLVideoElement> = {};
  let audioEls: Record<string, HTMLAudioElement> = {};

  export function registerVideoEl(clipId: string, el: HTMLVideoElement | null): void {
    if (el) videoEls[clipId] = el;
    else delete videoEls[clipId];
  }
  export function registerAudioEl(clipId: string, el: HTMLAudioElement | null): void {
    if (el) audioEls[clipId] = el;
    else delete audioEls[clipId];
  }

  // Return the set of clip ids that should be "active" (decoding) at a given time:
  // the clip under the playhead, plus the next clip (for seamless handoff).
  function activeClipIds(time: number): Set<string> {
    const p = $project;
    const ids = new Set<string>();
    for (const track of p.tracks) {
      if (track.type !== "video" || track.hidden) continue;
      const clips = [...track.clips].sort((a, b) => a.timelineStart - b.timelineStart);
      for (let i = 0; i < clips.length; i++) {
        const clip = clips[i];
        const dur = clip.sourceEnd - clip.sourceStart;
        const end = clip.timelineStart + dur;
        if (time >= clip.timelineStart && time < end) {
          ids.add(clip.id);
          // preload next clip for smooth transition
          if (i + 1 < clips.length) ids.add(clips[i + 1].id);
          break;
        }
        // if we're in a gap before this clip, preload it
        if (time < clip.timelineStart) {
          ids.add(clip.id);
          break;
        }
      }
    }
    // audio: include clips near playhead too
    for (const track of p.tracks) {
      if (track.type !== "audio" || track.hidden || track.muted) continue;
      for (const clip of track.clips) {
        const dur = clip.sourceEnd - clip.sourceStart;
        if (time >= clip.timelineStart - 0.1 && time < clip.timelineStart + dur) {
          ids.add(clip.id);
        }
      }
    }
    return ids;
  }

  function playAllMedia(): void {
    const active = activeClipIds(get(currentTime));
    for (const [id, el] of Object.entries(videoEls)) {
      if (active.has(id)) {
        if (el.paused) el.play().catch(() => {});
      } else {
        if (!el.paused) el.pause();
      }
    }
    for (const [id, el] of Object.entries(audioEls)) {
      if (active.has(id)) {
        if (el.paused) el.play().catch(() => {});
      } else {
        if (!el.paused) el.pause();
      }
    }
  }

  function pauseAllMedia(): void {
    for (const el of Object.values(videoEls)) el.pause();
    for (const el of Object.values(audioEls)) el.pause();
  }

  function syncMedia(time: number): void {
    const p = $project;
    const active = activeClipIds(time);
    // video clips
    for (const track of p.tracks) {
      if (track.type !== "video" || track.hidden) continue;
      for (const clip of track.clips) {
        const el = videoEls[clip.id];
        if (!el) continue;
        const dur = clip.sourceEnd - clip.sourceStart;
        const localTime = time - clip.timelineStart;
        if (active.has(clip.id) && localTime >= -0.05 && localTime <= dur) {
          const target = clip.sourceStart + Math.max(0, localTime);
          // tighter threshold while playing = smoother playback
          if (Math.abs(el.currentTime - target) > 0.3) {
            try {
              el.currentTime = target;
            } catch {
              /* not loaded yet */
            }
          }
        } else if (!el.paused) {
          el.pause();
        }
      }
    }
    // audio clips
    for (const track of p.tracks) {
      if (track.type !== "audio" || track.hidden || track.muted) continue;
      for (const clip of track.clips) {
        const el = audioEls[clip.id];
        if (!el) continue;
        const dur = clip.sourceEnd - clip.sourceStart;
        const localTime = time - clip.timelineStart;
        if (active.has(clip.id) && localTime >= -0.05 && localTime <= dur) {
          const target = clip.sourceStart + Math.max(0, localTime);
          if (Math.abs(el.currentTime - target) > 0.3) {
            try {
              el.currentTime = target;
            } catch {
              /* ignore */
            }
          }
        } else if (!el.paused) {
          el.pause();
        }
      }
    }
  }

  // sync whenever time changes due to user seeking
  currentTime.subscribe((time) => {
    if (!get(isPlaying)) syncMedia(time);
  });

  // ----- Derived: active clips (topmost video clip + active audios) -----
  $: activeVideo = computeActiveVideo($project, $currentTime);
  $: stageStyle = `aspect-ratio: ${$project.width} / ${$project.height};`;

  function computeActiveVideo(p: typeof $project, time: number): Clip | null {
    let found: Clip | null = null;
    for (let i = p.tracks.length - 1; i >= 0; i--) {
      const track = p.tracks[i];
      if (track.type !== "video" || track.hidden) continue;
      for (const clip of track.clips) {
        const dur = clip.sourceEnd - clip.sourceStart;
        if (time >= clip.timelineStart && time < clip.timelineStart + dur) {
          found = clip;
          break;
        }
      }
      if (found) break;
    }
    return found;
  }

  function filterStyle(clip: Clip): string {
    const b = 1 + clip.filters.brightness; // 0..2
    const c = 1 + clip.filters.contrast;
    const s = 1 + clip.filters.saturation;
    return `filter: brightness(${b}) contrast(${c}) saturate(${s});`;
  }

  // Seek by clicking on the scrubber
  function onScrub(e: MouseEvent): void {
    const el = e.currentTarget as HTMLElement;
    const rect = el.getBoundingClientRect();
    const ratio = (e.clientX - rect.left) / rect.width;
    const dur = get(duration);
    currentTime.set(Math.max(0, Math.min(dur, ratio * dur)));
  }
</script>

<svelte:window
  on:keydown={(e) => {
    if (e.code === "Space" && !(e.target as HTMLElement)?.matches("input,textarea,select")) {
      e.preventDefault();
      togglePlay();
    }
  }}
/>

<section class="panel preview">
  <div class="panel-header">
    <span>{$t("app.title")}</span>
    <span class="time-display">{formatTime($currentTime, true)} / {formatTime($duration, true)}</span>
  </div>
  <div class="stage-wrap">
    <div class="stage" style={stageStyle}>
      {#if !activeVideo}
        <div class="stage-empty">
          <Icon name="film" size={40} />
          <span>{$t("timeline.empty")}</span>
        </div>
      {/if}

      {#each $project.tracks as track (track.id)}
        {#if track.type === "video" && !track.hidden}
          {#each track.clips as clip (clip.id)}
            {@const asset = $project.assets[clip.mediaId]}
            {#if resolved[clip.mediaId] && asset}
              {#if asset.type === "image"}
                <img
                  class="layer"
                  style={`${filterStyle(clip)} ${activeVideo?.id === clip.id ? "" : "display:none;"}`}
                  src={resolved[clip.mediaId]}
                  alt={clip.name}
                  draggable="false"
                />
              {:else}
                <!-- eslint-disable-next-line svelte/no-dom-logging -->
                <video
                  class="layer"
                  style={`${filterStyle(clip)} ${activeVideo?.id === clip.id ? "" : "display:none;"}`}
                  src={resolved[clip.mediaId]}
                  playsinline
                  preload="auto"
                  bind:this={videoEls[clip.id]}
                  on:loadeddata={() => {
                    /* ready */
                  }}
                ></video>
              {/if}
            {/if}
          {/each}
        {/if}
      {/each}

      <!-- text overlay -->
      {#if activeVideo?.textOverlay?.text}
        <div
          class="text-overlay"
          style={`left:${activeVideo.textOverlay.x * 100}%; top:${activeVideo.textOverlay.y * 100}%; font-size:${activeVideo.textOverlay.fontSize * 100}%; color:${activeVideo.textOverlay.color}; font-weight:${activeVideo.textOverlay.bold ? 700 : 400};`}
        >
          {activeVideo.textOverlay.text}
        </div>
      {/if}

      <!-- hidden audio elements for separate audio tracks -->
      {#each $project.tracks as track (track.id)}
        {#if track.type === "audio"}
          {#each track.clips as clip (clip.id)}
            {@const asset = $project.assets[clip.mediaId]}
            {#if resolved[clip.mediaId] && asset}
              <audio
                src={resolved[clip.mediaId]}
                bind:this={audioEls[clip.id]}
                style="display:none;"
              ></audio>
            {/if}
          {/each}
        {/if}
      {/each}
    </div>
  </div>

  <div class="transport">
    <button class="icon" on:click={() => skip(-1)} title={$t("toolbar.skipEnd")}>
      <Icon name="skip-back" />
    </button>
    <button class="icon" on:click={() => stepFrame(-1)} title={$t("toolbar.previous")}>
      <Icon name="step-back" />
    </button>
    {#if $isPlaying}
      <button class="play-btn icon" on:click={pause} title={$t("toolbar.pause")}>
        <Icon name="pause" size={20} />
      </button>
    {:else}
      <button class="play-btn icon" on:click={play} title={$t("toolbar.play")}>
        <Icon name="play" size={20} />
      </button>
    {/if}
    <button class="icon" on:click={() => stepFrame(1)} title={$t("toolbar.next")}>
      <Icon name="step-forward" />
    </button>
    <button class="icon" on:click={() => skip(1)} title={$t("toolbar.skipStart")}>
      <Icon name="skip-forward" />
    </button>
  </div>

  <div class="scrubber" on:click={onScrub} role="slider" tabindex="0">
    <div class="scrub-fill" style={`width:${$duration > 0 ? ($currentTime / $duration) * 100 : 0}%;`}></div>
    <div class="scrub-handle" style={`left:${$duration > 0 ? ($currentTime / $duration) * 100 : 0}%;`}></div>
  </div>
</section>

<style>
  .preview {
    grid-area: preview;
  }
  .time-display {
    font-variant-numeric: tabular-nums;
    color: var(--text-dim);
    font-size: 12px;
  }
  .stage-wrap {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 12px;
    background:
      linear-gradient(45deg, #0d0d0f 25%, transparent 25%),
      linear-gradient(-45deg, #0d0d0f 25%, transparent 25%),
      linear-gradient(45deg, transparent 75%, #0d0d0f 75%),
      linear-gradient(-45deg, transparent 75%, #0d0d0f 75%);
    background-size: 20px 20px;
    background-position:
      0 0,
      0 10px,
      10px -10px,
      -10px 0;
    background-color: #15151a;
    overflow: hidden;
    min-height: 0;
  }
  .stage {
    position: relative;
    max-width: 100%;
    max-height: 100%;
    width: auto;
    height: 100%;
    background: #000;
    overflow: hidden;
    box-shadow: var(--shadow);
    display: flex;
  }
  .stage-empty {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 10px;
    color: var(--text-faint);
    font-size: 12px;
  }
  .layer {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: contain;
    pointer-events: none;
  }
  .text-overlay {
    position: absolute;
    transform: translate(-50%, -50%);
    white-space: pre-wrap;
    text-align: center;
    text-shadow: 0 1px 3px rgba(0, 0, 0, 0.8);
    pointer-events: none;
    font-family: sans-serif;
    line-height: 1.2;
  }
  .transport {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 4px;
    padding: 6px;
    border-top: 1px solid var(--border-soft);
    flex-shrink: 0;
  }
  .play-btn {
    width: 40px;
    height: 40px;
    background: var(--accent);
    color: white;
    border-radius: 50%;
  }
  .play-btn:hover {
    background: var(--accent-hover);
  }
  .scrubber {
    position: relative;
    height: 18px;
    padding: 7px 0;
    cursor: pointer;
    flex-shrink: 0;
  }
  .scrubber::before {
    content: "";
    position: absolute;
    left: 0;
    right: 0;
    top: 50%;
    transform: translateY(-50%);
    height: 4px;
    background: var(--border);
    border-radius: 2px;
  }
  .scrub-fill {
    position: absolute;
    left: 0;
    top: 50%;
    transform: translateY(-50%);
    height: 4px;
    background: var(--accent);
    border-radius: 2px;
    pointer-events: none;
  }
  .scrub-handle {
    position: absolute;
    top: 50%;
    transform: translate(-50%, -50%);
    width: 12px;
    height: 12px;
    border-radius: 50%;
    background: white;
    border: 2px solid var(--accent);
    pointer-events: none;
  }
</style>
