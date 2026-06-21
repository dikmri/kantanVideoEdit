This directory holds the bundled FFmpeg and FFprobe binaries.

During local development this folder may be empty (the app falls back to a
system-installed FFmpeg). The CI release workflow downloads the platform
appropriate ffmpeg/ffprobe binaries into this directory before building so they
are bundled with the installer for end users.

Files placed here are copied into the app's resource directory at runtime and
discovered by the Rust backend (resolve_ffmpeg).
