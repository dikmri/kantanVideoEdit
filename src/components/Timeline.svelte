<script lang="ts">
  import {
    project,
    currentTime,
    duration,
    selectedClipId,
    zoom,
    isPlaying,
    addTrack,
    deleteTrack,
    addClipToTrack,
    splitAt,
    projectStore,
    commitHistory,
  } from "../stores";
  import { assetUrl } from "../lib/api";
  import { formatTime, clamp } from "../lib/util";
  import { t } from "../i18n";
  import { get } from "svelte/store";
  import { onMount } from "svelte";
  import Icon from "./Icon.svelte";
  import type { Clip, Track } from "../types";

  const HEADER_W = 150;
  const TRACK_H = 64;
  const RULER_H = 28;

  // thumbnail cache
  const thumbCache = new Map<string, string>();
  async function getThumb(mediaId: string): Promise<string | null> {
    if (thumbCache.has(mediaId)) return thumbCache.get(mediaId)!;
    const asset = $project.assets[mediaId];
    if (!asset) return null;
    const p = asset.thumbnail || asset.path;
    try {
      const url = await assetUrl(p);
      thumbCache.set(mediaId, url);
      return url;
    } catch {
      return null;
    }
  }

  // ---------- Coordinate helpers ----------
  function xToTime(x: number): number {
    return Math.max(0, (x + scrollLeft) / $zoom);
  }

  // ---------- Ruler ----------
  let totalDuration = 0;
  let scrollLeft = 0;
  let viewportW = 800;

  $: totalDuration = $duration;

  function niceStep(): number {
    const targetPx = 80;
    const targetTime = targetPx / $zoom;
    const steps = [0.5, 1, 2, 5, 10, 15, 30, 60, 120, 300, 600];
    for (const s of steps) if (s >= targetTime) return s;
    return 1200;
  }

  $: rulerTicks = (() => {
    const step = niceStep();
    const ticks: { time: number; label: string }[] = [];
    const max = Math.max(totalDuration, viewportW / $zoom) + step;
    for (let t = 0; t <= max; t += step) {
      ticks.push({ time: t, label: formatTime(t) });
    }
    return ticks;
  })();

  $: contentWidth = Math.max(viewportW, (totalDuration + 5) * $zoom) + 100;

  function onRulerClick(e: MouseEvent): void {
    const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
    const x = e.clientX - rect.left;
    const time = xToTime(x);
    isPlaying.set(false);
    currentTime.set(clamp(time, 0, Math.max(0, totalDuration)));
  }

  // ---------- Drag interactions ----------
  type DragState = {
    type: "move" | "trim-start" | "trim-end";
    clipId: string;
    trackId: string;
    startX: number;
    origSourceStart: number;
    origSourceEnd: number;
    origTimelineStart: number;
    moveTargetTrackId?: string;
  };
  let drag: DragState | null = null;

  function startDrag(
    e: PointerEvent,
    clip: Clip,
    trackId: string,
    type: DragState["type"],
  ): void {
    if (trackId === trackId && get(project).tracks.find((t) => t.id === trackId)?.locked) return;
    e.preventDefault();
    e.stopPropagation();
    selectedClipId.set(clip.id);
    drag = {
      type,
      clipId: clip.id,
      trackId,
      startX: e.clientX,
      origSourceStart: clip.sourceStart,
      origSourceEnd: clip.sourceEnd,
      origTimelineStart: clip.timelineStart,
    };
    window.addEventListener("pointermove", onDragMove);
    window.addEventListener("pointerup", onDragEnd);
  }

  function onDragMove(e: PointerEvent): void {
    if (!drag) return;
    const dx = (e.clientX - drag.startX) / $zoom;
    if (drag.type === "move") {
      let newStart = Math.max(0, drag.origTimelineStart + dx);
      const target = trackFromY(e.clientY);
      const targetTrackId = target?.id ?? drag.trackId;
      drag.moveTargetTrackId = targetTrackId;
      projectStore.updateSilent((proj) => {
        let moved: Clip | null = null;
        for (const tr of proj.tracks) {
          const idx = tr.clips.findIndex((c) => c.id === drag!.clipId);
          if (idx >= 0) {
            moved = tr.clips.splice(idx, 1)[0];
            break;
          }
        }
        if (moved) {
          moved.timelineStart = newStart;
          const dest = proj.tracks.find((x) => x.id === targetTrackId);
          if (dest && !dest.locked) {
            dest.clips.push(moved);
            dest.clips.sort((a, b) => a.timelineStart - b.timelineStart);
          } else {
            // put back to original
            const orig = proj.tracks.find((x) => x.id === drag!.trackId);
            orig?.clips.push(moved);
            orig?.clips.sort((a, b) => a.timelineStart - b.timelineStart);
          }
        }
        return { ...proj };
      });
    } else if (drag.type === "trim-start") {
      const newSourceStart = clamp(
        drag.origSourceStart + dx,
        0,
        drag.origSourceEnd - 0.05,
      );
      const delta = newSourceStart - drag.origSourceStart;
      projectStore.updateSilent((proj) => {
        for (const tr of proj.tracks) {
          const clip = tr.clips.find((c) => c.id === drag!.clipId);
          if (!clip) continue;
          if (clip.isImage) {
            clip.timelineStart = Math.max(0, drag.origTimelineStart + dx);
          } else {
            clip.sourceStart = newSourceStart;
            clip.timelineStart = Math.max(0, drag.origTimelineStart + delta);
          }
          break;
        }
        return { ...proj };
      });
    } else if (drag.type === "trim-end") {
      projectStore.updateSilent((proj) => {
        for (const tr of proj.tracks) {
          const clip = tr.clips.find((c) => c.id === drag!.clipId);
          if (!clip) continue;
          if (clip.isImage) {
            const origDur = drag.origSourceEnd - drag.origSourceStart;
            const newDur = Math.max(0.1, origDur + dx);
            clip.sourceStart = 0;
            clip.sourceEnd = newDur;
          } else {
            clip.sourceEnd = Math.max(drag.origSourceStart + 0.05, drag.origSourceEnd + dx);
          }
          break;
        }
        return { ...proj };
      });
    }
  }

  function onDragEnd(): void {
    window.removeEventListener("pointermove", onDragMove);
    window.removeEventListener("pointerup", onDragEnd);
    if (drag) {
      commitHistory();
      drag = null;
    }
  }

  // Track lanes Y positions
  let trackLanesEl: HTMLElement | null = null;
  function trackFromY(clientY: number): Track | null {
    if (!trackLanesEl) return null;
    const rect = trackLanesEl.getBoundingClientRect();
    const y = clientY - rect.top;
    const p = get(project);
    let acc = 0;
    for (const tr of p.tracks) {
      if (y >= acc && y < acc + TRACK_H) return tr;
      acc += TRACK_H;
    }
    return null;
  }

  // ---------- Drop from MediaPanel ----------
  function onLaneDragOver(e: DragEvent): void {
    if (e.dataTransfer?.types.includes("application/x-kve-asset")) {
      e.preventDefault();
      e.dataTransfer.dropEffect = "copy";
    }
  }

  function onLaneDrop(e: DragEvent): void {
    const assetId = e.dataTransfer?.getData("application/x-kve-asset");
    if (!assetId || !trackLanesEl) return;
    e.preventDefault();
    const target = trackFromY(e.clientY);
    if (!target || target.locked) return;
    const asset = get(project).assets[assetId];
    if (!asset) return;
    const isVideoLike = asset.type === "video" || asset.type === "image";
    if (target.type === "video" && !isVideoLike) return;
    if (target.type === "audio" && asset.type !== "audio") return;
    const rect = trackLanesEl.getBoundingClientRect();
    const x = e.clientX - rect.left - HEADER_W + scrollLeft;
    const time = Math.max(0, x / $zoom);
    addClipToTrack(target.id, asset, time);
    commitHistory();
  }

  // ---------- Track toggles ----------
  function toggle(track: Track, key: "muted" | "hidden" | "locked"): void {
    projectStore.update((p) => {
      const t = p.tracks.find((x) => x.id === track.id);
      if (t) t[key] = !t[key];
      return { ...p };
    });
    commitHistory();
  }

  // ---------- Zoom ----------
  function zoomIn(): void {
    zoom.update((z) => clamp(z * 1.3, 10, 600));
  }
  function zoomOut(): void {
    zoom.update((z) => clamp(z / 1.3, 10, 600));
  }
  function fitZoom(): void {
    if (totalDuration <= 0) return;
    zoom.set(clamp((viewportW - 20) / totalDuration, 10, 600));
  }

  // ---------- Layout measurement ----------
  let scrollContainer: HTMLElement | null = null;
  function measure(): void {
    if (scrollContainer) {
      viewportW = scrollContainer.clientWidth;
      scrollLeft = scrollContainer.scrollLeft;
    }
  }
  onMount(() => {
    requestAnimationFrame(measure);
  });
</script>

<svelte:window on:resize={measure} />

<section class="panel timeline">
  <div class="panel-header">
    <span>{$t("timeline.title")}</span>
    <div class="tl-tools">
      <button class="icon" on:click={zoomOut} title={$t("view.zoomOut")}><Icon name="zoom-out" size={14} /></button>
      <button class="icon" on:click={zoomIn} title={$t("view.zoomIn")}><Icon name="zoom-in" size={14} /></button>
      <button class="icon" on:click={fitZoom} title={$t("view.fit")}><Icon name="sparkles" size={14} /></button>
      <div class="divider"></div>
      <button on:click={() => addTrack("video")} title={$t("timeline.addVideoTrack")}>
        <Icon name="film" size={13} /> <Icon name="plus" size={12} />
      </button>
      <button on:click={() => addTrack("audio")} title={$t("timeline.addAudioTrack")}>
        <Icon name="music" size={13} /> <Icon name="plus" size={12} />
      </button>
    </div>
  </div>

  <div class="tl-scroll" bind:this={scrollContainer} on:scroll={measure}>
    <!-- Ruler -->
    <div class="ruler-row">
      <div class="ruler-corner" style="width:{HEADER_W}px;height:{RULER_H}px;">
        <button
          class="icon split-btn"
          on:click={() => { splitAt($currentTime); commitHistory(); }}
          title={$t("toolbar.split")}
        >
          <Icon name="scissors" size={14} />
        </button>
      </div>
      <div class="ruler" style="width:{contentWidth}px;height:{RULER_H}px;" on:click={onRulerClick}>
        {#each rulerTicks as tick}
          <div class="tick" style="left:{tick.time * $zoom}px;">
            <div class="tick-mark"></div>
            <span class="tick-label">{tick.label}</span>
          </div>
        {/each}
      </div>
    </div>

    <!-- Tracks -->
    <div class="tracks-area" bind:this={trackLanesEl}>
      {#if $project.tracks.length === 0}
        <div class="tl-empty">{$t("timeline.empty")}</div>
      {/if}
      {#each $project.tracks as track (track.id)}
        <div class="track" style="height:{TRACK_H}px;" class:locked={track.locked}>
          <div class="track-header" style="width:{HEADER_W}px;height:{TRACK_H}px;">
            <div class="track-type-badge {track.type}">
              <Icon name={track.type === "video" ? "film" : "music"} size={12} />
              <span>{track.name}</span>
            </div>
            <div class="track-actions">
              {#if track.type === "video"}
                <button class="icon mini" class:active={track.hidden} on:click={() => toggle(track, "hidden")} title={$t("timeline.hide")}>
                  <Icon name={track.hidden ? "eye-off" : "eye"} size={13} />
                </button>
              {/if}
              <button class="icon mini" class:active={track.muted} on:click={() => toggle(track, "muted")} title={$t("timeline.mute")}>
                <Icon name={track.muted ? "volume-off" : "volume"} size={13} />
              </button>
              <button class="icon mini" class:active={track.locked} on:click={() => toggle(track, "locked")} title={$t("timeline.lock")}>
                <Icon name={track.locked ? "lock" : "unlock"} size={13} />
              </button>
              <button class="icon mini danger" on:click={() => deleteTrack(track.id)} title={$t("common.delete")}>
                <Icon name="trash" size={12} />
              </button>
            </div>
          </div>
          <div
            class="lane"
            style="width:{contentWidth}px;height:{TRACK_H}px;"
            on:dragover={onLaneDragOver}
            on:drop={onLaneDrop}
          >
            {#each track.clips as clip (clip.id)}
              {@const dur = clip.sourceEnd - clip.sourceStart}
              {@const left = clip.timelineStart * $zoom}
              {@const width = Math.max(8, dur * $zoom)}
              {@const asset = $project.assets[clip.mediaId]}
              <div
                class="clip {track.type}"
                class:selected={$selectedClipId === clip.id}
                style="left:{left}px;width:{width}px;"
                on:pointerdown={(e) => startDrag(e, clip, track.id, "move")}
                on:click={(e) => { e.stopPropagation(); selectedClipId.set(clip.id); }}
                role="button"
                tabindex="0"
              >
                {#if track.type === "video" && asset?.type !== "audio"}
                  <div class="clip-thumbs">
                    {#await getThumb(clip.mediaId)}{:then url}{#if url}<img src={url} alt="" draggable="false" />{/if}{/await}
                  </div>
                {/if}
                <div class="clip-label">{clip.name}</div>
                <div class="clip-handle left" on:pointerdown={(e) => startDrag(e, clip, track.id, "trim-start")}></div>
                <div class="clip-handle right" on:pointerdown={(e) => startDrag(e, clip, track.id, "trim-end")}></div>
                {#if clip.volume !== 1}
                  <span class="clip-tag">{Math.round(clip.volume * 100)}%</span>
                {/if}
                {#if clip.textOverlay?.text}
                  <span class="clip-tag type">T</span>
                {/if}
              </div>
            {/each}
          </div>
        </div>
      {/each}
    </div>

    <!-- Playhead -->
    <div class="playhead" style="left:{$currentTime * $zoom + HEADER_W - scrollLeft}px;">
      <div class="playhead-head"></div>
    </div>
  </div>
</section>

<style>
  .timeline {
    grid-area: timeline;
    overflow: hidden;
  }
  .tl-tools {
    display: flex;
    align-items: center;
    gap: 2px;
  }
  .tl-tools .divider {
    width: 1px;
    height: 18px;
    background: var(--border);
    margin: 0 4px;
  }
  .tl-scroll {
    flex: 1;
    overflow: auto;
    position: relative;
    background: var(--bg);
  }
  .ruler-row {
    display: flex;
    position: sticky;
    top: 0;
    z-index: 5;
  }
  .ruler-corner {
    position: sticky;
    left: 0;
    z-index: 6;
    background: var(--bg-panel);
    border-bottom: 1px solid var(--border);
    border-right: 1px solid var(--border);
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
  }
  .split-btn {
    background: var(--accent-soft);
    color: var(--accent);
  }
  .ruler {
    position: relative;
    background: var(--bg-panel);
    border-bottom: 1px solid var(--border);
    cursor: pointer;
    flex-shrink: 0;
  }
  .tick {
    position: absolute;
    top: 0;
    bottom: 0;
  }
  .tick-mark {
    position: absolute;
    bottom: 0;
    left: 0;
    width: 1px;
    height: 8px;
    background: var(--text-faint);
  }
  .tick-label {
    position: absolute;
    top: 4px;
    left: 4px;
    font-size: 10px;
    color: var(--text-dim);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }
  .tracks-area {
    position: relative;
  }
  .tl-empty {
    padding: 40px;
    text-align: center;
    color: var(--text-faint);
    width: 100%;
  }
  .track {
    display: flex;
    border-bottom: 1px solid var(--border-soft);
  }
  .track-header {
    position: sticky;
    left: 0;
    z-index: 4;
    background: var(--bg-panel);
    border-right: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    justify-content: center;
    gap: 4px;
    padding: 4px 8px;
    flex-shrink: 0;
  }
  .track.locked .track-header {
    opacity: 0.7;
  }
  .track-type-badge {
    display: flex;
    align-items: center;
    gap: 5px;
    font-size: 11px;
    font-weight: 600;
    color: var(--text-dim);
  }
  .track-type-badge.video {
    color: var(--video-color);
  }
  .track-type-badge.audio {
    color: var(--audio-color);
  }
  .track-actions {
    display: flex;
    gap: 2px;
  }
  .mini {
    width: 22px;
    height: 22px;
    padding: 0;
  }
  .mini.active {
    color: var(--accent);
    background: var(--accent-soft);
  }
  .lane {
    position: relative;
    background: var(--bg);
    flex-shrink: 0;
  }
  .clip {
    position: absolute;
    top: 4px;
    height: 56px;
    border-radius: 5px;
    overflow: hidden;
    cursor: grab;
    display: flex;
    align-items: center;
    user-select: none;
    border: 2px solid transparent;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.3);
    color: white;
  }
  .clip.video {
    background: linear-gradient(180deg, #3a72d6, #2f5fb5);
  }
  .clip.audio {
    background: linear-gradient(180deg, #2ea866, #248a52);
  }
  .clip.selected {
    border-color: white;
    box-shadow: 0 0 0 2px var(--accent), 0 2px 6px rgba(0, 0, 0, 0.4);
    z-index: 2;
  }
  .clip-thumbs {
    position: absolute;
    inset: 0;
    display: flex;
    opacity: 0.45;
    overflow: hidden;
  }
  .clip-thumbs img {
    height: 100%;
    width: auto;
    object-fit: cover;
  }
  .clip-label {
    position: relative;
    z-index: 1;
    padding: 0 8px;
    font-size: 11px;
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    text-shadow: 0 1px 2px rgba(0, 0, 0, 0.6);
    min-width: 0;
  }
  .clip-handle {
    position: absolute;
    top: 0;
    bottom: 0;
    width: 8px;
    cursor: ew-resize;
    z-index: 3;
    transition: background 0.12s;
  }
  .clip-handle:hover {
    background: rgba(255, 255, 255, 0.3);
  }
  .clip-handle.left {
    left: 0;
  }
  .clip-handle.right {
    right: 0;
  }
  .clip-tag {
    margin-left: auto;
    margin-right: 6px;
    padding: 0 4px;
    font-size: 9px;
    background: rgba(0, 0, 0, 0.4);
    border-radius: 3px;
    z-index: 1;
    flex-shrink: 0;
  }
  .playhead {
    position: absolute;
    top: 0;
    bottom: 0;
    width: 2px;
    background: var(--danger);
    z-index: 7;
    pointer-events: none;
    margin-left: -1px;
  }
  .playhead-head {
    position: absolute;
    top: 0;
    left: -6px;
    width: 14px;
    height: 14px;
    background: var(--danger);
    clip-path: polygon(0 0, 100% 0, 50% 100%);
  }
</style>
