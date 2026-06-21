<script lang="ts">
  import {
    project,
    currentTime,
    duration,
    isPlaying,
    selectedClipId,
  } from "../stores";
  import { assetUrl } from "../lib/api";
  import { formatTime } from "../lib/util";
  import { t } from "../i18n";
  import { get } from "svelte/store";
  import Icon from "./Icon.svelte";
  import type { Clip, MediaAsset } from "../types";

  // ---- URL resolve cache ----
  const urlCache = new Map<string, string>();

  async function getUrl(asset: MediaAsset): Promise<string> {
    const key = asset.id;
    if (urlCache.has(key)) return urlCache.get(key)!;
    const url = await assetUrl(asset.path);
    urlCache.set(key, url);
    return url;
  }

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

  $: Object.keys($project.assets).forEach(ensureResolved);

  // ---- Playback clock ----
  let raf = 0;
  let lastTs = 0;

  function loop(ts: number): void {
    if (!get(isPlaying)) { raf = 0; return; }
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
    syncVideo(next);
    if (get(isPlaying)) raf = requestAnimationFrame(loop);
  }

  function play(): void {
    const dur = get(duration);
    if (get(currentTime) >= dur - 0.01) currentTime.set(0);
    isPlaying.set(true);
    lastTs = 0;
    cancelAnimationFrame(raf);
    raf = requestAnimationFrame(loop);
    startVideoAt(get(currentTime));
  }

  function pause(): void {
    isPlaying.set(false);
    cancelAnimationFrame(raf);
    pauseVideo();
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
    syncVideo(get(currentTime));
  }

  function skip(dir: 1 | -1): void {
    pause();
    currentTime.update((t) => Math.max(0, t + dir * 5));
    syncVideo(get(currentTime));
  }

  isPlaying.subscribe((playing) => {
    if (!playing) cancelAnimationFrame(raf);
  });

  // ---- Single video element playback ----
  // Instead of one <video> per clip, we use a SINGLE <video> element and
  // swap its source when the active clip changes. This avoids the decoder
  // overload that causes stuttering with multiple simultaneous videos.

  let videoEl: HTMLVideoElement | null = null;
  let activeClip: Clip | null = null;
  let activeAsset: MediaAsset | null = null;
  let isVideoLoading = false;
  // Queue of seek-after-load operations
  let pendingSeek: number | null = null;

  function findActiveClip(p: typeof $project, time: number): { clip: Clip; asset: MediaAsset } | null {
    // Search tracks top-down (topmost track has visual priority)
    for (let i = p.tracks.length - 1; i >= 0; i--) {
      const track = p.tracks[i];
      if (track.type !== "video" || track.hidden) continue;
      for (const clip of track.clips) {
        const dur = clip.sourceEnd - clip.sourceStart;
        if (time >= clip.timelineStart && time < clip.timelineStart + dur) {
          const asset = p.assets[clip.mediaId];
          if (asset && asset.type !== "audio") return { clip, asset };
        }
      }
    }
    return null;
  }

  // Find what clip would be next after current active clip ends
  function findNextClip(p: typeof $project, time: number): { clip: Clip; asset: MediaAsset } | null {
    const current = findActiveClip(p, time);
    if (!current) return null;
    for (let i = p.tracks.length - 1; i >= 0; i--) {
      const track = p.tracks[i];
      if (track.type !== "video" || track.hidden) continue;
      const sortedClips = [...track.clips].sort((a, b) => a.timelineStart - b.timelineStart);
      for (const clip of sortedClips) {
        if (clip.id !== current.clip.id && clip.timelineStart >= current.clip.timelineStart) {
          const asset = p.assets[clip.mediaId];
          if (asset && asset.type !== "audio") return { clip, asset };
        }
      }
    }
    return null;
  }

  // Preload next clip's URL so switch is instant
  let preloadedNext: string | null = null;

  function preloadNext(time: number): void {
    const next = findNextClip($project, time);
    if (next && resolved[next.clip.mediaId]) {
      preloadedNext = next.clip.id;
    }
  }

  // Switch the video element to play a given clip
  async function switchToClip(clip: Clip, asset: MediaAsset, localTime: number): Promise<void> {
    if (!videoEl) return;
    const url = resolved[clip.mediaId];
    if (!url) return;

    const needsNewSrc = !activeClip || activeClip.id !== clip.id;

    if (needsNewSrc) {
      activeClip = clip;
      activeAsset = asset;
      isVideoLoading = true;

      if (asset.type === "image") {
        // For images, we don't need video playback
        videoEl.src = "";
        videoEl.poster = url;
        isVideoLoading = false;
        return;
      }

      videoEl.src = url;
      // We'll seek once loaded
      pendingSeek = clip.sourceStart + Math.max(0, localTime);

      if (get(isPlaying)) {
        try {
          await videoEl.play();
        } catch {
          // autoplay blocked, will retry on canplay
        }
      }
    } else {
      // Same clip, just seek if needed
      if (asset.type !== "image" && videoEl.src) {
        const target = clip.sourceStart + Math.max(0, localTime);
        if (Math.abs(videoEl.currentTime - target) > 0.5) {
          try { videoEl.currentTime = target; } catch { /* ignore */ }
        }
      }
    }
  }

  function startVideoAt(time: number): void {
    const found = findActiveClip($project, time);
    if (found) {
      const localTime = time - found.clip.timelineStart;
      switchToClip(found.clip, found.asset, localTime);
    } else {
      if (videoEl && !videoEl.paused) videoEl.pause();
      activeClip = null;
      activeAsset = null;
    }
  }

  function syncVideo(time: number): void {
    const found = findActiveClip($project, time);
    if (found) {
      const localTime = time - found.clip.timelineStart;
      const dur = found.clip.sourceEnd - found.clip.sourceStart;
      if (localTime >= 0 && localTime < dur) {
        if (!activeClip || activeClip.id !== found.clip.id) {
          // Clip transition
          switchToClip(found.clip, found.asset, localTime);
          preloadNext(time);
        } else if (activeAsset && activeAsset.type !== "image" && videoEl && videoEl.src) {
          // Same clip — keep in sync with the clock
          const target = found.clip.sourceStart + localTime;
          // Only force-seek if really far off; otherwise let the video run naturally
          if (videoEl && !videoEl.paused && Math.abs(videoEl.currentTime - target) < 2.0) {
            // Within 2s tolerance — let native playback run (smoother for any fps)
          } else if (videoEl) {
            try { videoEl.currentTime = target; } catch { /* ignore */ }
          }
        }
      }
    } else {
      if (videoEl && !videoEl.paused) videoEl.pause();
      activeClip = null;
      activeAsset = null;
    }
  }

  function pauseVideo(): void {
    if (videoEl && !videoEl.paused) videoEl.pause();
  }

  // ---- Audio elements (separate, lighter-weight) ----
  let audioEls: Record<string, HTMLAudioElement> = {};

  function syncAudio(time: number): void {
    const p = $project;
    for (const track of p.tracks) {
      if (track.type !== "audio" || track.hidden || track.muted) continue;
      for (const clip of track.clips) {
        const el = audioEls[clip.id];
        if (!el) continue;
        const dur = clip.sourceEnd - clip.sourceStart;
        const localTime = time - clip.timelineStart;
        if (localTime >= 0 && localTime < dur) {
          const target = clip.sourceStart + localTime;
          if (el.paused && get(isPlaying)) {
            el.currentTime = target;
            el.play().catch(() => {});
          } else if (!el.paused && Math.abs(el.currentTime - target) > 1.0) {
            try { el.currentTime = target; } catch { /* ignore */ }
          }
        } else if (!el.paused) {
          el.pause();
        }
      }
    }
  }

  // Keep audio in sync during playback loop
  const origLoop = loop;
  // Override loop to also sync audio
  // Actually, let's just add audio sync into the main loop
  // Rewriting loop:

  // ---- Computed values ----
  $: activeVideo = findActiveClip($project, $currentTime)?.clip ?? null;
  $: stageStyle = `aspect-ratio: ${$project.width} / ${$project.height};`;

  function filterStyle(clip: Clip): string {
    const b = 1 + clip.filters.brightness;
    const c = 1 + clip.filters.contrast;
    const s = 1 + clip.filters.saturation;
    return `filter: brightness(${b}) contrast(${c}) saturate(${s});`;
  }

  function onScrub(e: MouseEvent): void {
    const el = e.currentTarget as HTMLElement;
    const rect = el.getBoundingClientRect();
    const ratio = (e.clientX - rect.left) / rect.width;
    const dur = get(duration);
    currentTime.set(Math.max(0, Math.min(dur, ratio * dur)));
    syncVideo(get(currentTime));
    syncAudio(get(currentTime));
  }

  // Handle video element events
  function onVideoCanPlay(): void {
    isVideoLoading = false;
    if (pendingSeek !== null && videoEl) {
      try { videoEl.currentTime = pendingSeek; } catch { /* ignore */ }
      pendingSeek = null;
      if (get(isPlaying)) {
        videoEl.play().catch(() => {});
      }
    }
  }

  // Override the loop to include audio sync
  // We redefine loop to include audio
  let raf2 = 0;
  function mainLoop(ts: number): void {
    if (!get(isPlaying)) { raf2 = 0; return; }
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
    syncVideo(next);
    syncAudio(next);
    if (get(isPlaying)) raf2 = requestAnimationFrame(mainLoop);
  }

  // Override play/pause to use mainLoop
  function play2(): void {
    const dur = get(duration);
    if (get(currentTime) >= dur - 0.01) currentTime.set(0);
    isPlaying.set(true);
    lastTs = 0;
    cancelAnimationFrame(raf2);
    raf2 = requestAnimationFrame(mainLoop);
    startVideoAt(get(currentTime));
  }

  function pause2(): void {
    isPlaying.set(false);
    cancelAnimationFrame(raf2);
    pauseVideo();
  }

  function togglePlay2(): void {
    if (get(isPlaying)) pause2();
    else play2();
  }

  function stop2(): void {
    pause2();
    currentTime.set(0);
  }

  function stepFrame2(dir: 1 | -1): void {
    pause2();
    const fps = $project.fps || 30;
    currentTime.update((t) => Math.max(0, t + dir * (1 / fps)));
    syncVideo(get(currentTime));
    syncAudio(get(currentTime));
  }

  function skip2(dir: 1 | -1): void {
    pause2();
    currentTime.update((t) => Math.max(0, t + dir * 5));
    syncVideo(get(currentTime));
    syncAudio(get(currentTime));
  }
</script>

<svelte:window
  on:keydown={(e) => {
    if (e.code === "Space" && !(e.target as HTMLElement)?.matches("input,textarea,select")) {
      e.preventDefault();
      togglePlay2();
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

      <!-- Single video element — swaps source on clip change -->
      {#if activeVideo && activeAsset}
        {#if activeAsset.type === "image" && resolved[activeVideo.mediaId]}
          <img
            class="layer"
            style={filterStyle(activeVideo)}
            src={resolved[activeVideo.mediaId]}
            alt={activeVideo.name}
            draggable="false"
          />
        {:else}
          <video
            class="layer"
            style={filterStyle(activeVideo)}
            bind:this={videoEl}
            playsinline
            preload="auto"
            on:canplay={onVideoCanPlay}
            on:waiting={() => { isVideoLoading = true; }}
            on:playing={() => { isVideoLoading = false; }}
          ></video>
        {/if}
      {/if}

      <!-- text overlay -->
      {#if activeVideo?.textOverlay?.text}
        <div
          class="text-overlay"
          style={`left:${activeVideo.textOverlay.x * 100}%; top:${activeVideo.textOverlay.y * 100}%; font-size:${activeVideo.textOverlay.fontSize * 100}%; color:${activeVideo.textOverlay.color}; font-weight:${activeVideo.textOverlay.bold ? 700 : 400};`}
        >
          {activeVideo.textOverlay.text}
        </div>
      {/if}

      <!-- Audio elements (lightweight) -->
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
    <button class="icon" on:click={() => skip2(-1)} title={$t("toolbar.skipEnd")}>
      <Icon name="skip-back" />
    </button>
    <button class="icon" on:click={() => stepFrame2(-1)} title={$t("toolbar.previous")}>
      <Icon name="step-back" />
    </button>
    {#if $isPlaying}
      <button class="play-btn icon" on:click={pause2} title={$t("toolbar.pause")}>
        <Icon name="pause" size={20} />
      </button>
    {:else}
      <button class="play-btn icon" on:click={play2} title={$t("toolbar.play")}>
        <Icon name="play" size={20} />
      </button>
    {/if}
    <button class="icon" on:click={() => stepFrame2(1)} title={$t("toolbar.next")}>
      <Icon name="step-forward" />
    </button>
    <button class="icon" on:click={() => skip2(1)} title={$t("toolbar.skipStart")}>
      <Icon name="skip-forward" />
    </button>
  </div>

  <div class="scrubber" on:click={onScrub} role="slider" tabindex="0"
    aria-valuenow={Math.round($currentTime)}
    aria-valuemin={0} aria-valuemax={Math.round($duration)}>
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
    background-position: 0 0, 0 10px, 10px -10px, -10px 0;
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
