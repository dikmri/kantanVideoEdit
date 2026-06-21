# Kantan Video Edit

**Simple. Fast. Lightweight.** — a cross-platform, multilingual video editor built with Tauri 2, Svelte 5, and FFmpeg.

カンタン動画編集は、Tauri 2 + Svelte 5 + FFmpeg で構築された、シンプルで高速・軽量なクロスプラットフォーム対応の多言語動画編集ソフトです。

![Kantan Video Edit](src-tauri/icons/icon.png)

## ✨ Features

- 🎬 **Multi-track timeline** — video & audio tracks, drag, trim, split, reorder
- ✂️ **Editing** — cut/split at playhead, trim handles, duplicate, delete, undo/redo
- 🎚️ **Per-clip adjustments** — volume, brightness, contrast, saturation
- 🔤 **Text overlays** — positioned, sized, colored captions
- 🌗 **Fades** — fade in / fade out for video and audio
- 🖼️ **Media library** — import video, audio, and images with thumbnails
- ▶️ **Live preview** — real-time playback with CSS-filtered preview
- 🌍 **Multilingual UI** — English, 日本語, Español, Français, Deutsch, 中文
- 🎨 **Dark / Light themes**
- ⚡ **Fast FFmpeg export** — H.264/H.265/VP9, AAC/MP3/Opus, CRF quality control, custom resolution & framerate
- 💾 **Project save/load** — native `.kveproj` project files
- 📦 **Bundled FFmpeg** — no external dependencies for end users (release builds)

## 🚀 Download

Pre-built binaries for **Windows**, **macOS** (Intel & Apple Silicon), and **Linux** are published on the [Releases](../../releases) page. Download, extract, and run — FFmpeg is bundled.

## 🛠️ Development

### Prerequisites

- [Node.js](https://nodejs.org/) 20+
- [Rust](https://www.rust-lang.org/tools/install) (stable)
- [FFmpeg](https://ffmpeg.org/) in your `PATH` (for dev)
- Platform-specific Tauri dependencies:
  - **Linux:** `libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf libxdo-dev libssl-dev`
  - **macOS:** Xcode command-line tools
  - **Windows:** Microsoft Visual Studio C++ Build Tools

### Run

```bash
npm install
npm run tauri:dev
```

### Build

```bash
npm run tauri:build
```

Artifacts are placed in `src-tauri/target/release/bundle/`.

## 🏗️ Architecture

```
┌─────────────────────────────────────────────┐
│  Frontend (Svelte 5 + TypeScript + Vite)    │
│  UI, timeline, preview, i18n, state         │
└──────────────────────┬──────────────────────┘
                       │ Tauri IPC (invoke / events)
┌──────────────────────┴──────────────────────┐
│  Backend (Rust)                             │
│  ffprobe (metadata) · ffmpeg (render)       │
│  filter_complex graph builder               │
└─────────────────────────────────────────────┘
```

The Rust backend builds an FFmpeg `filter_complex` graph from the timeline
(scaling, `eq` color filters, `fade`, `drawtext` overlays, `overlay` compositing,
`amix`/`adelay` audio) and streams render progress back to the UI via events.

## ⌨️ Keyboard Shortcuts

| Action | Shortcut |
|---|---|
| Play / Pause | `Space` |
| Split at playhead | `S` |
| Delete clip | `Del` |
| Undo / Redo | `Ctrl+Z` / `Ctrl+Y` |
| Save / Open / New | `Ctrl+S` / `Ctrl+O` / `Ctrl+N` |
| Export | `Ctrl+E` |
| Import | `Ctrl+I` |
| Duplicate | `Ctrl+D` |
| Zoom in / out | `+` / `-` |

## 📦 Releases

Releases are automated via [GitHub Actions](.github/workflows/release.yml).
Push a `v*` tag to build and publish installers for all platforms with FFmpeg bundled.

```bash
git tag v1.0.0
git push origin v1.0.0
```

## 📄 License

MIT — see [LICENSE](LICENSE).
