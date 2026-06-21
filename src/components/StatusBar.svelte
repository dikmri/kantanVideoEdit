<script lang="ts">
  import { project, duration, dirty, selectedClipId } from "../stores";
  import { get } from "svelte/store";
  import { formatTime } from "../lib/util";
  import { t } from "../i18n";
  import Icon from "./Icon.svelte";

  $: clipCount = $project.tracks.reduce((n, t) => n + t.clips.length, 0);
  $: trackCount = $project.tracks.length;
</script>

<footer class="statusbar">
  <div class="status-left">
    <span class="status-item" class:dirty={$dirty}>
      <span class="dot"></span>
      {$dirty ? "●" : $t("status.ready")}
    </span>
    <span class="sep">|</span>
    <span class="status-item">{$project.name}</span>
    <span class="sep">|</span>
    <span class="status-item">{$project.width}×{$project.height} · {$project.fps}fps</span>
  </div>
  <div class="status-right">
    <span class="status-item">{$t("status.duration")}: {formatTime($duration)}</span>
    <span class="sep">|</span>
    <span class="status-item">{$t("status.clips", { count: clipCount })}</span>
    <span class="sep">|</span>
    <span class="status-item">{$t("status.tracks", { count: trackCount })}</span>
    {#if $selectedClipId}
      <span class="sep">|</span>
      <span class="status-item accent">{$t("properties.clip")}: ●</span>
    {/if}
  </div>
</footer>

<style>
  .statusbar {
    height: var(--status-h);
    background: var(--bg-elevated);
    border-top: 1px solid var(--border-soft);
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 12px;
    font-size: 11px;
    color: var(--text-dim);
    gap: 8px;
  }
  .status-left,
  .status-right {
    display: flex;
    align-items: center;
    gap: 6px;
    overflow: hidden;
    white-space: nowrap;
  }
  .status-item {
    display: flex;
    align-items: center;
    gap: 4px;
    font-variant-numeric: tabular-nums;
  }
  .status-item.accent {
    color: var(--accent);
  }
  .status-item.dirty .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--warning);
  }
  .sep {
    color: var(--border);
  }
</style>
