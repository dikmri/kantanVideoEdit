// Core data model types for Kantan Video Edit

export type MediaType = "video" | "audio" | "image";

export interface MediaAsset {
  id: string;
  path: string;
  name: string;
  type: MediaType;
  duration: number; // seconds (images default to a still duration)
  width?: number;
  height?: number;
  thumbnail?: string; // file path or data url
  codec?: string;
  fps?: number;
}

export interface ClipFilters {
  brightness: number; // -1.0 .. 1.0 (0 = neutral)
  contrast: number; // -1.0 .. 1.0
  saturation: number; // -1.0 .. 1.0
}

export interface TextOverlay {
  text: string;
  x: number; // 0..1 relative
  y: number; // 0..1 relative
  fontSize: number; // relative to canvas height
  color: string; // hex
  bold: boolean;
}

export interface Clip {
  id: string;
  mediaId: string;
  name: string;
  sourceStart: number; // seconds into source
  sourceEnd: number; // seconds into source
  timelineStart: number; // seconds on timeline
  volume: number; // 0..2 (1 = original)
  filters: ClipFilters;
  textOverlay: TextOverlay | null;
  fadeIn: number; // seconds
  fadeOut: number; // seconds
  // image-only
  isImage?: boolean;
}

export type TrackType = "video" | "audio";

export interface Track {
  id: string;
  type: TrackType;
  name: string;
  clips: Clip[];
  muted: boolean;
  hidden: boolean;
  locked: boolean;
}

export interface Project {
  name: string;
  width: number;
  height: number;
  fps: number;
  tracks: Track[];
  assets: Record<string, MediaAsset>;
}

export interface ExportSettings {
  outputPath: string;
  format: "mp4" | "webm" | "mov" | "mkv";
  videoCodec: "libx264" | "libx265" | "libvpx-vp9" | "copy";
  audioCodec: "aac" | "mp3" | "opus" | "copy";
  crf: number; // 0..51
  preset: "ultrafast" | "superfast" | "veryfast" | "faster" | "fast" | "medium" | "slow" | "slower";
  width?: number; // optional override
  height?: number;
  fps?: number;
}

export interface ProjectFile {
  version: number;
  project: Project;
}
