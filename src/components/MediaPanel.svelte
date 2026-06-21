<script lang="ts">
  import { open, message as messageDialog } from "@tauri-apps/plugin-dialog";
  import { project, addMediaToProject, removeMedia, addClipToTrack, addTrack } from "../stores";
  import { probeMediaBatch, assetUrl } from "../lib/api";
  import { stableId, formatTime } from "../lib/util";
  import { t } from "../i18n";
  import Icon from "./Icon.svelte";
  import type { MediaAsset } from "../types";

  let importing = false;
  let dragOver = false;
  // cache of asset -> resolved url for thumbnails / preview
  const urlCache = new Map<string, string>();

  const MEDIA_FILTERS = [
    {
      name: "Media files",
      extensions: [
        "mp4", "mov", "mkv", "avi", "webm", "flv", "wmv", "m4v", "mpg", "mpeg", "ts",
        "mp3", "wav", "aac", "flac", "ogg", "m4a", "wma",
        "png", "jpg", "jpeg", "bmp", "webp", "gif", "tiff",
      ],
    },
  ];

  async function importFiles(): Promise<void> {
    if (importing) return;
    importing = true;
    try {
      const result = await open({ multiple: true, filters: MEDIA_FILTERS });
      if (!result) return;
      const paths = Array.isArray(result) ? result : [result];
      await importPaths(paths);
    } finally {
      importing = false;
    }
  }

  async function importPaths(paths: string[]): Promise<void> {
    if (paths.length === 0) return;
    try {
      const results = await probeMediaBatch(paths);
      for (let i = 0; i < paths.length; i++) {
        const probe = results[i];
        if (!probe) continue;
        const asset: MediaAsset = {
          id: stableId("asset"),
          path: paths[i],
          name: basename(paths[i]),
          type: probe.type,
          duration: probe.duration,
          width: probe.width,
          height: probe.height,
          codec: probe.codec,
          fps: probe.fps,
          thumbnail: probe.thumbnail,
        };
        addMediaToProject(asset);
      }
    } catch (err) {
      console.error("Import failed", err);
      await messageDialog(`${$t("status.importing")} : ${String(err)}`, { kind: "error" });
    }
  }

  function basename(p: string): string {
    const norm = p.replace(/\\/g, "/");
    const parts = norm.split("/");
    return parts[parts.length - 1] || p;
  }

  async function getThumb(asset: MediaAsset): Promise<string | null> {
    if (asset.type === "audio") return null;
    if (urlCache.has(asset.id)) return urlCache.get(asset.id)!;
    let p = asset.thumbnail || asset.path;
    try {
      const url = await assetUrl(p);
      urlCache.set(asset.id, url);
      return url;
    } catch {
      return null;
    }
  }

  function onDragStart(e: DragEvent, asset: MediaAsset): void {
    e.dataTransfer?.setData("application/x-kve-asset", asset.id);
    e.dataTransfer!.effectAllowed = "copy";
  }

  function addToTimeline(asset: MediaAsset): void {
    const p = $project;
    const trackType = asset.type === "audio" ? "audio" : "video";
    let track = p.tracks.find((t) => t.type === trackType);
    if (!track) {
      addTrack(trackType);
      track = $project.tracks.find((t) => t.type === trackType);
    }
    if (track) addClipToTrack(track.id, asset);
  }

  function handleDrop(e: DragEvent): void {
    e.preventDefault();
    dragOver = false;
    const files = e.dataTransfer?.files;
    if (files && files.length > 0) {
      // Tauri exposes real paths via path property
      const paths: string[] = [];
      for (let i = 0; i < files.length; i++) {
        const f = files[i] as unknown as { path?: string };
        if (f.path) paths.push(f.path);
      }
      if (paths.length) importPaths(paths);
    }
  }

  // reactive list
  $: assets = Object.values($project.assets);
</script>

<section class="panel media" data-panel="media">
  <div class="panel-header">
    <span>{$t("media.title")}</span>
    <button class="icon" on:click={importFiles} title={$t("media.import")} disabled={importing}>
      <Icon name="import" size={14} />
    </button>
  </div>
  <div
    class="panel-body media-body"
    class:drag-over={dragOver}
    on:dragover|preventDefault={() => (dragOver = true)}
    on:dragleave={() => (dragOver = false)}
    on:drop={handleDrop}
  >
    {#if assets.length === 0}
      <button class="empty-state" on:click={importFiles} disabled={importing}>
        <Icon name="import" size={28} />
        <span>{importing ? $t("status.importing") : $t("media.empty")}</span>
      </button>
    {:else}
      <div class="media-grid">
        {#each assets as asset (asset.id)}
          <div
            class="media-card"
            draggable="true"
            on:dragstart={(e) => onDragStart(e, asset)}
            on:dblclick={() => addToTimeline(asset)}
            tabindex="0"
            role="button"
          >
            <div class="thumb">
              {#if asset.type === "audio"}
                <Icon name="music" size={24} />
              {:else}
                {#await getThumb(asset)}
                  <div class="thumb-loading"></div>
                {:then url}
                  {#if url}
                    <!-- eslint-disable-next-line svelte/no-at-html-tags -->
                    <img src={url} alt={asset.name} />
                  {:else}
                    <Icon name={asset.type === "image" ? "image" : "film"} size={24} />
                  {/if}
                {/await}
              {/if}
              <span class="type-badge {asset.type}">{asset.type}</span>
              {#if asset.duration > 0}
                <span class="dur-badge">{formatTime(asset.duration)}</span>
              {/if}
            </div>
            <div class="media-name" title={asset.name}>{asset.name}</div>
            <button
              class="remove-btn icon"
              on:click|stopPropagation={() => removeMedia(asset.id)}
              title={$t("media.remove")}
            >
              <Icon name="trash" size={12} />
            </button>
          </div>
        {/each}
      </div>
    {/if}
  </div>
  <div class="panel-footer">
    <span class="hint">{$t("media.empty").includes("ドラッグ") ? "ドラッグ&ドロップで追加" : "Drag to timeline"}</span>
  </div>
</section>

<style>
  .media-body {
    padding: 8px;
  }
  .media-body.drag-over {
    background: var(--accent-soft);
    outline: 2px dashed var(--accent);
    outline-offset: -8px;
  }
  .empty-state {
    width: 100%;
    height: 100%;
    min-height: 200px;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 10px;
    color: var(--text-dim);
    border: 2px dashed var(--border);
    border-radius: var(--radius);
    background: transparent;
  }
  .media-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(110px, 1fr));
    gap: 8px;
  }
  .media-card {
    position: relative;
    background: var(--bg-input);
    border: 1px solid var(--border-soft);
    border-radius: var(--radius-sm);
    overflow: hidden;
    cursor: grab;
    transition: border-color 0.12s, transform 0.08s;
  }
  .media-card:hover {
    border-color: var(--accent);
  }
  .media-card:active {
    cursor: grabbing;
    transform: scale(0.98);
  }
  .thumb {
    width: 100%;
    aspect-ratio: 16 / 9;
    background: #000;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-faint);
    position: relative;
    overflow: hidden;
  }
  .thumb img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .thumb-loading {
    width: 24px;
    height: 24px;
    border: 2px solid var(--border);
    border-top-color: var(--accent);
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  .type-badge {
    position: absolute;
    top: 4px;
    left: 4px;
    font-size: 9px;
    text-transform: uppercase;
    background: rgba(0, 0, 0, 0.7);
    color: white;
    padding: 1px 5px;
    border-radius: 3px;
  }
  .type-badge.video {
    background: var(--video-color);
  }
  .type-badge.audio {
    background: var(--audio-color);
  }
  .type-badge.image {
    background: var(--image-color);
  }
  .dur-badge {
    position: absolute;
    bottom: 4px;
    right: 4px;
    font-size: 10px;
    background: rgba(0, 0, 0, 0.75);
    color: white;
    padding: 1px 5px;
    border-radius: 3px;
  }
  .media-name {
    padding: 4px 6px;
    font-size: 11px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .remove-btn {
    position: absolute;
    top: 2px;
    right: 2px;
    width: 20px;
    height: 20px;
    opacity: 0;
    background: rgba(0, 0, 0, 0.6);
    color: white;
    transition: opacity 0.12s;
  }
  .media-card:hover .remove-btn {
    opacity: 1;
  }
  .panel-footer {
    padding: 4px 10px;
    border-top: 1px solid var(--border-soft);
    font-size: 10px;
    color: var(--text-faint);
    text-align: center;
  }
</style>
