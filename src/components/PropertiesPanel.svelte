<script lang="ts">
  import {
    project,
    selectedClipId,
    updateClip,
    trimClip,
    deleteClip,
    duplicateClip,
    currentTime,
    commitHistory,
  } from "../stores";
  import { get } from "svelte/store";
  import { t } from "../i18n";
  import { formatTime } from "../lib/util";
  import Icon from "./Icon.svelte";
  import type { Clip, Track, TextOverlay } from "../types";

  $: selected = findSelected($project, $selectedClipId);

  function findSelected(p: typeof $project, id: string | null): { clip: Clip; track: Track } | null {
    if (!id) return null;
    for (const track of p.tracks) {
      const clip = track.clips.find((c) => c.id === id);
      if (clip) return { clip, track };
    }
    return null;
  }

  function patch(p: Partial<Clip>): void {
    if (!selected) return;
    updateClip(selected.clip.id, p);
  }

  function patchFilters(key: keyof Clip["filters"], value: number): void {
    if (!selected) return;
    updateClip(selected.clip.id, {
      filters: { ...selected.clip.filters, [key]: value },
    });
  }

  // Debounced commit for slider drags
  let commitTimer: ReturnType<typeof setTimeout> | null = null;
  function scheduleCommit(): void {
    if (commitTimer) clearTimeout(commitTimer);
    commitTimer = setTimeout(() => commitHistory(), 400);
  }

  function toggleText(): void {
    if (!selected) return;
    if (selected.clip.textOverlay) {
      patch({ textOverlay: null });
    } else {
      const overlay: TextOverlay = {
        text: "",
        x: 0.5,
        y: 0.85,
        fontSize: 0.06,
        color: "#ffffff",
        bold: true,
      };
      patch({ textOverlay: overlay });
    }
  }

  function patchText(p: Partial<TextOverlay>): void {
    if (!selected?.clip.textOverlay) return;
    patch({ textOverlay: { ...selected.clip.textOverlay, ...p } });
  }

  function resetFilters(): void {
    patch({ filters: { brightness: 0, contrast: 0, saturation: 0 } });
    commitHistory();
  }

  function setSourceToPlayhead(side: "start" | "end"): void {
    if (!selected) return;
    const t = get(currentTime);
    if (side === "start") {
      trimClip(selected.clip.id, "start", Math.min(t, selected.clip.sourceEnd - 0.1));
    } else {
      trimClip(selected.clip.id, "end", Math.max(t, selected.clip.sourceStart + 0.1));
    }
    commitHistory();
  }
</script>

<section class="panel properties">
  <div class="panel-header">
    <span>{$t("properties.title")}</span>
    {#if selected}
      <div class="header-actions">
        <button class="icon" on:click={() => duplicateClip(selected!.clip.id)} title={$t("edit.duplicate")}>
          <Icon name="copy" size={13} />
        </button>
        <button class="icon danger" on:click={() => deleteClip(selected!.clip.id)} title={$t("edit.delete")}>
          <Icon name="trash" size={13} />
        </button>
      </div>
    {/if}
  </div>
  <div class="panel-body">
    {#if !selected}
      <div class="empty">
        <Icon name="settings" size={28} />
        <p>{$t("properties.none")}</p>
      </div>
    {:else}
      {@const c = selected.clip}
      <div class="group">
        <div class="group-title">{$t("properties.clip")}</div>
        <div class="clip-name" title={c.name}>{c.name}</div>
        <div class="info-grid">
          <div><span>{$t("properties.duration")}</span><b>{formatTime(c.sourceEnd - c.sourceStart)}</b></div>
          <div><span>{$t("properties.timelineStart")}</span><b>{formatTime(c.timelineStart)}</b></div>
        </div>
      </div>

      {#if selected.track.type === "audio" || $project.assets[c.mediaId]?.type !== "image"}
        <div class="group">
          <label class="field">
            <div class="field-row">
              <span class="field-label">{$t("properties.volume")}</span>
              <span class="field-value">{Math.round(c.volume * 100)}%</span>
            </div>
            <input
              type="range"
              min="0"
              max="2"
              step="0.01"
              value={c.volume}
              on:input={(e) => {
                patch({ volume: parseFloat(e.currentTarget.value) });
                scheduleCommit();
              }}
            />
          </label>
        </div>
      {/if}

      {#if selected.track.type === "video"}
        <div class="group">
          <div class="group-title">
            <span>{$t("properties.brightness")}</span>
            <button class="text-btn" on:click={resetFilters}>{$t("properties.reset")}</button>
          </div>
          <label class="field">
            <div class="field-row">
              <span class="field-label">{$t("properties.brightness")}</span>
              <span class="field-value">{c.filters.brightness > 0 ? "+" : ""}{Math.round(c.filters.brightness * 100)}</span>
            </div>
            <input
              type="range"
              min="-1"
              max="1"
              step="0.01"
              value={c.filters.brightness}
              on:input={(e) => {
                patchFilters("brightness", parseFloat(e.currentTarget.value));
                scheduleCommit();
              }}
            />
          </label>
          <label class="field">
            <div class="field-row">
              <span class="field-label">{$t("properties.contrast")}</span>
              <span class="field-value">{c.filters.contrast > 0 ? "+" : ""}{Math.round(c.filters.contrast * 100)}</span>
            </div>
            <input
              type="range"
              min="-1"
              max="1"
              step="0.01"
              value={c.filters.contrast}
              on:input={(e) => {
                patchFilters("contrast", parseFloat(e.currentTarget.value));
                scheduleCommit();
              }}
            />
          </label>
          <label class="field">
            <div class="field-row">
              <span class="field-label">{$t("properties.saturation")}</span>
              <span class="field-value">{c.filters.saturation > 0 ? "+" : ""}{Math.round(c.filters.saturation * 100)}</span>
            </div>
            <input
              type="range"
              min="-1"
              max="1"
              step="0.01"
              value={c.filters.saturation}
              on:input={(e) => {
                patchFilters("saturation", parseFloat(e.currentTarget.value));
                scheduleCommit();
              }}
            />
          </label>
        </div>

        <div class="group">
          <div class="group-title">{$t("properties.fadeIn")} / {$t("properties.fadeOut")}</div>
          <label class="field">
            <div class="field-row">
              <span class="field-label">{$t("properties.fadeIn")}</span>
              <span class="field-value">{c.fadeIn.toFixed(2)}{$t("common.seconds")}</span>
            </div>
            <input
              type="range"
              min="0"
              max="3"
              step="0.05"
              value={c.fadeIn}
              on:input={(e) => {
                patch({ fadeIn: parseFloat(e.currentTarget.value) });
                scheduleCommit();
              }}
            />
          </label>
          <label class="field">
            <div class="field-row">
              <span class="field-label">{$t("properties.fadeOut")}</span>
              <span class="field-value">{c.fadeOut.toFixed(2)}{$t("common.seconds")}</span>
            </div>
            <input
              type="range"
              min="0"
              max="3"
              step="0.05"
              value={c.fadeOut}
              on:input={(e) => {
                patch({ fadeOut: parseFloat(e.currentTarget.value) });
                scheduleCommit();
              }}
            />
          </label>
        </div>

        <div class="group">
          <div class="group-title">
            <span>{$t("properties.textOverlay")}</span>
            <label class="switch">
              <input type="checkbox" checked={!!c.textOverlay} on:change={toggleText} />
              <span>{$t("properties.enableText")}</span>
            </label>
          </div>
          {#if c.textOverlay}
            <textarea
              placeholder={$t("properties.textPlaceholder")}
              value={c.textOverlay.text}
              on:input={(e) => patchText({ text: e.currentTarget.value })}
              rows="2"
            ></textarea>
            <div class="text-controls">
              <label class="mini-field">
                <span>{$t("properties.fontSize")}</span>
                <input
                  type="range"
                  min="0.02"
                  max="0.2"
                  step="0.005"
                  value={c.textOverlay.fontSize}
                  on:input={(e) => patchText({ fontSize: parseFloat(e.currentTarget.value) })}
                />
              </label>
              <label class="mini-field color-field">
                <span>{$t("properties.color")}</span>
                <input
                  type="color"
                  value={c.textOverlay.color}
                  on:input={(e) => patchText({ color: e.currentTarget.value })}
                />
              </label>
              <label class="mini-field">
                <span>X</span>
                <input
                  type="range"
                  min="0"
                  max="1"
                  step="0.01"
                  value={c.textOverlay.x}
                  on:input={(e) => patchText({ x: parseFloat(e.currentTarget.value) })}
                />
              </label>
              <label class="mini-field">
                <span>Y</span>
                <input
                  type="range"
                  min="0"
                  max="1"
                  step="0.01"
                  value={c.textOverlay.y}
                  on:input={(e) => patchText({ y: parseFloat(e.currentTarget.value) })}
                />
              </label>
              <label class="switch">
                <input
                  type="checkbox"
                  checked={c.textOverlay.bold}
                  on:change={(e) => patchText({ bold: e.currentTarget.checked })}
                />
                <span>{$t("properties.bold")}</span>
              </label>
            </div>
            <button class="text-btn" on:click={() => { scheduleCommit(); }}>{$t("common.apply")}</button>
          {/if}
        </div>
      {/if}

      <div class="group">
        <div class="group-title">{$t("properties.sourceStart")} / {$t("properties.sourceEnd")}</div>
        <div class="trim-row">
          <div>
            <div class="field-label">{$t("properties.sourceStart")}</div>
            <div class="field-value mono">{formatTime(c.sourceStart)}</div>
            <button class="mini-btn" on:click={() => setSourceToPlayhead("start")}>→</button>
          </div>
          <div>
            <div class="field-label">{$t("properties.sourceEnd")}</div>
            <div class="field-value mono">{formatTime(c.sourceEnd)}</div>
            <button class="mini-btn" on:click={() => setSourceToPlayhead("end")}>→</button>
          </div>
        </div>
      </div>
    {/if}
  </div>
</section>

<style>
  .properties {
    grid-area: properties;
  }
  .header-actions {
    display: flex;
    gap: 2px;
  }
  .empty {
    height: 100%;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 10px;
    color: var(--text-faint);
    text-align: center;
    padding: 20px;
    font-size: 12px;
  }
  .group {
    padding: 10px 12px;
    border-bottom: 1px solid var(--border-soft);
  }
  .group-title {
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--text-dim);
    margin-bottom: 8px;
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  .clip-name {
    font-size: 13px;
    font-weight: 600;
    margin-bottom: 6px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .info-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 6px;
    font-size: 11px;
  }
  .info-grid div {
    display: flex;
    flex-direction: column;
    background: var(--bg-input);
    padding: 4px 6px;
    border-radius: var(--radius-sm);
  }
  .info-grid span {
    color: var(--text-faint);
    font-size: 10px;
  }
  .info-grid b {
    font-weight: 600;
    font-variant-numeric: tabular-nums;
  }
  .field {
    display: block;
    margin-bottom: 10px;
  }
  .field:last-child {
    margin-bottom: 0;
  }
  .field-row {
    display: flex;
    justify-content: space-between;
    margin-bottom: 4px;
  }
  .field-label {
    font-size: 11px;
    color: var(--text-dim);
  }
  .field-value {
    font-size: 11px;
    font-variant-numeric: tabular-nums;
    color: var(--text);
  }
  .mono {
    font-family: ui-monospace, "SF Mono", Consolas, monospace;
  }
  textarea {
    width: 100%;
    resize: vertical;
    min-height: 40px;
    margin-bottom: 8px;
  }
  .text-controls {
    display: grid;
    gap: 6px;
  }
  .mini-field {
    display: grid;
    grid-template-columns: 56px 1fr;
    align-items: center;
    gap: 6px;
    font-size: 11px;
    color: var(--text-dim);
  }
  .color-field {
    grid-template-columns: 56px 36px;
  }
  .switch {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    font-size: 11px;
    color: var(--text-dim);
    cursor: pointer;
    text-transform: none;
    letter-spacing: 0;
    font-weight: 400;
  }
  .text-btn {
    font-size: 11px;
    padding: 4px 8px;
    color: var(--accent);
    text-transform: none;
    letter-spacing: 0;
    font-weight: 400;
  }
  .trim-row {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
  }
  .trim-row > div {
    background: var(--bg-input);
    border-radius: var(--radius-sm);
    padding: 6px;
    text-align: center;
  }
  .mini-btn {
    margin-top: 4px;
    padding: 2px 8px;
    background: var(--accent-soft);
    color: var(--accent);
    font-size: 11px;
  }
</style>
