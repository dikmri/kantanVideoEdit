<script lang="ts">
  import { createEventDispatcher } from "svelte";
  import { save } from "@tauri-apps/plugin-dialog";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { project } from "../stores";
  import { renderProject, checkFfmpeg } from "../lib/api";
  import { t } from "../i18n";
  import Icon from "./Icon.svelte";
  import type { ExportSettings } from "../types";

  const dispatch = createEventDispatcher();

  let format: ExportSettings["format"] = "mp4";
  let videoCodec: ExportSettings["videoCodec"] = "libx264";
  let audioCodec: ExportSettings["audioCodec"] = "aac";
  let crf = 20;
  let preset: ExportSettings["preset"] = "medium";
  let useCustomResolution = false;
  let width = 1920;
  let height = 1080;
  let fps = 30;
  let outputPath = "";
  let exporting = false;
  let progress = 0;
  let stage = "";
  let error = "";
  let success = false;
  let ffmpegStatus = "checking";
  let unlistenFn: UnlistenFn | null = null;

  async function check(): Promise<void> {
    try {
      const res = await checkFfmpeg();
      ffmpegStatus = res.available ? "ok" : "missing";
    } catch {
      ffmpegStatus = "missing";
    }
  }
  check();

  async function browse(): Promise<void> {
    const ext = format;
    const path = await save({
      defaultPath: `${$project.name || "output"}.${ext}`,
      filters: [{ name: format.toUpperCase(), extensions: [ext] }],
    });
    if (path) outputPath = path;
  }

  function effectiveSettings(): ExportSettings {
    return {
      outputPath,
      format,
      videoCodec,
      audioCodec,
      crf,
      preset,
      width: useCustomResolution ? width : undefined,
      height: useCustomResolution ? height : undefined,
      fps,
    };
  }

  async function start(): Promise<void> {
    if (!outputPath) {
      await browse();
      if (!outputPath) return;
    }
    exporting = true;
    progress = 0;
    stage = $t("export.exporting");
    error = "";
    success = false;

    unlistenFn = await listen<{ progress: number; stage: string; done: boolean; error?: string }>(
      "kve://render-progress",
      (event) => {
        const payload = event.payload;
        progress = payload.progress;
        if (payload.stage) stage = payload.stage;
        if (payload.error) {
          error = payload.error;
          exporting = false;
          cleanup();
        } else if (payload.done) {
          success = true;
          exporting = false;
          progress = 1;
          cleanup();
        }
      },
    );

    try {
      await renderProject($project, effectiveSettings());
    } catch (err) {
      error = String(err);
      exporting = false;
      cleanup();
    }
  }

  function cleanup(): void {
    if (unlistenFn) {
      unlistenFn();
      unlistenFn = null;
    }
  }

  function close(): void {
    if (exporting) return;
    cleanup();
    dispatch("close");
  }

  function onFormatChange(e: Event): void {
    format = (e.currentTarget as HTMLSelectElement).value as ExportSettings["format"];
    // adjust sensible defaults
    if (format === "webm") {
      videoCodec = "libvpx-vp9";
      audioCodec = "opus";
    } else if (format === "mp4") {
      videoCodec = "libx264";
      audioCodec = "aac";
    }
  }
</script>

<div class="overlay" on:click|self={close} on:keydown={(e) => e.key === "Escape" && close()} role="presentation">
  <div class="dialog" role="dialog" aria-modal="true">
    <div class="dialog-header">
      <h2><Icon name="export" size={18} /> {$t("export.title")}</h2>
      <button class="icon" on:click={close} disabled={exporting}><Icon name="close" size={16} /></button>
    </div>

    <div class="dialog-body">
      {#if ffmpegStatus === "missing"}
        <div class="warning-box">
          <Icon name="info" size={16} />
          <span>FFmpeg not detected. Install FFmpeg or the bundled engine will be used at export.</span>
        </div>
      {/if}

      {#if error}
        <div class="error-box">
          <Icon name="info" size={16} />
          <span>{$t("export.error")}: {error}</span>
        </div>
      {/if}

      {#if success}
        <div class="success-box">
          <Icon name="check" size={16} />
          <span>{$t("export.success")}</span>
        </div>
      {/if}

      <div class="form-grid">
        <label class="form-field">
          <span>{$t("export.format")}</span>
          <select value={format} on:change={onFormatChange}>
            <option value="mp4">MP4</option>
            <option value="webm">WebM</option>
            <option value="mov">MOV</option>
            <option value="mkv">MKV</option>
          </select>
        </label>

        <label class="form-field">
          <span>{$t("export.videoCodec")}</span>
          <select bind:value={videoCodec}>
            <option value="libx264">H.264 (libx264)</option>
            <option value="libx265">H.265 (libx265)</option>
            <option value="libvpx-vp9">VP9</option>
          </select>
        </label>

        <label class="form-field">
          <span>{$t("export.audioCodec")}</span>
          <select bind:value={audioCodec}>
            <option value="aac">AAC</option>
            <option value="mp3">MP3</option>
            <option value="opus">Opus</option>
          </select>
        </label>

        <label class="form-field">
          <span>{$t("export.fps")}</span>
          <select bind:value={fps}>
            <option value={24}>24</option>
            <option value={25}>25</option>
            <option value={30}>30</option>
            <option value={50}>50</option>
            <option value={60}>60</option>
          </select>
        </label>

        <label class="form-field full">
          <div class="field-head">
            <span>{$t("export.quality")} — CRF {crf}</span>
            <span class="hint">{crf <= 18 ? $t("export.high") : $t("export.low")}</span>
          </div>
          <input type="range" min="0" max="51" bind:value={crf} disabled={exporting} />
        </label>

        <label class="form-field full">
          <div class="field-head">
            <span>{$t("export.preset")}</span>
          </div>
          <select bind:value={preset} disabled={exporting}>
            <option value="ultrafast">Ultrafast</option>
            <option value="superfast">Superfast</option>
            <option value="veryfast">Veryfast</option>
            <option value="faster">Faster</option>
            <option value="fast">Fast</option>
            <option value="medium">Medium</option>
            <option value="slow">Slow</option>
            <option value="slower">Slower</option>
          </select>
        </label>

        <label class="form-field full check">
          <input type="checkbox" bind:checked={useCustomResolution} disabled={exporting} />
          <span>{$t("export.resolution")} ({$project.width}×{$project.height})</span>
        </label>
        {#if useCustomResolution}
          <label class="form-field">
            <span>Width</span>
            <input type="number" bind:value={width} min="16" step="2" disabled={exporting} />
          </label>
          <label class="form-field">
            <span>Height</span>
            <input type="number" bind:value={height} min="16" step="2" disabled={exporting} />
          </label>
        {/if}

        <div class="form-field full output-row">
          <span>{$t("export.output")}</span>
          <div class="output-input">
            <input type="text" bind:value={outputPath} placeholder="output.{format}" disabled={exporting} />
            <button on:click={browse} disabled={exporting}><Icon name="folder" size={14} /> {$t("export.browse")}</button>
          </div>
        </div>
      </div>

      {#if exporting || success}
        <div class="progress-area">
          <div class="progress-bar">
            <div class="progress-fill" style="width:{Math.round(progress * 100)}%;"></div>
          </div>
          <div class="progress-text">
            <span>{stage || $t("export.exporting")}</span>
            <span>{Math.round(progress * 100)}%</span>
          </div>
        </div>
      {/if}
    </div>

    <div class="dialog-footer">
      <button on:click={close} disabled={exporting}>{$t("export.cancel")}</button>
      <button class="primary" on:click={start} disabled={exporting || ffmpegStatus === "checking"}>
        <Icon name="export" size={14} /> {$t("export.start")}
      </button>
    </div>
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.55);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 200;
    backdrop-filter: blur(2px);
  }
  .dialog {
    background: var(--bg-elevated);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    width: min(560px, 92vw);
    max-height: 90vh;
    display: flex;
    flex-direction: column;
    box-shadow: var(--shadow);
  }
  .dialog-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 12px 16px;
    border-bottom: 1px solid var(--border-soft);
  }
  .dialog-header h2 {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 15px;
    font-weight: 600;
  }
  .dialog-body {
    padding: 16px;
    overflow: auto;
  }
  .form-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 12px;
  }
  .form-field {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 12px;
    color: var(--text-dim);
  }
  .form-field.full {
    grid-column: 1 / -1;
  }
  .form-field.check {
    flex-direction: row;
    align-items: center;
    gap: 6px;
  }
  .field-head {
    display: flex;
    justify-content: space-between;
  }
  .hint {
    font-size: 10px;
    color: var(--text-faint);
  }
  .output-input {
    display: flex;
    gap: 6px;
  }
  .output-input input {
    flex: 1;
  }
  .warning-box,
  .error-box,
  .success-box {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 10px;
    border-radius: var(--radius-sm);
    font-size: 12px;
    margin-bottom: 12px;
  }
  .warning-box {
    background: rgba(255, 177, 61, 0.12);
    color: var(--warning);
  }
  .error-box {
    background: rgba(255, 92, 92, 0.12);
    color: var(--danger);
  }
  .success-box {
    background: rgba(61, 220, 132, 0.12);
    color: var(--success);
  }
  .progress-area {
    margin-top: 16px;
  }
  .progress-bar {
    height: 8px;
    background: var(--bg-input);
    border-radius: 4px;
    overflow: hidden;
    border: 1px solid var(--border);
  }
  .progress-fill {
    height: 100%;
    background: linear-gradient(90deg, var(--accent), var(--accent-hover));
    transition: width 0.2s;
  }
  .progress-text {
    display: flex;
    justify-content: space-between;
    margin-top: 6px;
    font-size: 11px;
    color: var(--text-dim);
  }
  .dialog-footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    padding: 12px 16px;
    border-top: 1px solid var(--border-soft);
  }
</style>
