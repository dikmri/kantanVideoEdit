import { invoke } from "@tauri-apps/api/core";
import { convertFileSrc } from "@tauri-apps/api/core";
import type { ExportSettings, MediaAsset, Project } from "../types";

export interface ProbeResult {
  duration: number;
  width?: number;
  height?: number;
  type: "video" | "audio" | "image";
  codec?: string;
  fps?: number;
  thumbnail?: string;
}

/** Probe a media file for metadata and optional thumbnail extraction. */
export async function probeMedia(path: string): Promise<ProbeResult> {
  return await invoke<ProbeResult>("probe_media", { path });
}

/** Probe multiple files in batch (faster). */
export async function probeMediaBatch(paths: string[]): Promise<ProbeResult[]> {
  return await invoke<ProbeResult[]>("probe_media_batch", { paths });
}

export interface RenderProgress {
  progress: number; // 0..1
  stage: string;
  done: boolean;
  error?: string;
}

/** Render the project to a file using FFmpeg. Returns the output path on completion (via events). */
export async function renderProject(
  project: Project,
  settings: ExportSettings,
): Promise<void> {
  await invoke("render_project", { project, settings });
}

/** Check whether a bundled or system FFmpeg is available. */
export async function checkFfmpeg(): Promise<{ available: boolean; version: string; path: string }> {
  return await invoke("check_ffmpeg");
}

/** Open a path in the OS file manager / reveal in finder. */
export async function revealInFolder(path: string): Promise<void> {
  await invoke("reveal_in_folder", { path });
}

/** Convert a local file path into a usable asset URL for the webview (via Tauri asset protocol). */
export async function assetUrl(path: string): Promise<string> {
  return convertFileSrc(path);
}

export { MediaAsset };
