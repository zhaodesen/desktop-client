# Muyu

A cross-platform floating subtitle player for local audio, language practice, and fully offline transcription.

[English](./README.md) | [简体中文](./README.zh-CN.md)

## Features

- Import local audio files
- Load `.srt` and `.vtt` subtitle files
- Display subtitles in a floating desktop window
- Loop a single subtitle line for focused listening practice
- Generate subtitles locally with `ffmpeg` and `whisper-cli`
- Download the default Whisper model from inside the app
- Package platform sidecars for macOS and Windows releases

## Tech Stack

Tauri 2 · Rust · Svelte 5 · TypeScript · Vite · ffmpeg · whisper.cpp · yt-dlp

## Getting Started

### Prerequisites

- Node.js and npm
- Rust stable
- `ffmpeg`, `whisper-cli`, and `yt-dlp`, either on `PATH` or prepared as Tauri sidecars
- A compatible Whisper model such as `ggml-base.bin`

### Development

```bash
npm install
npm run tauri dev
```

The application checks environment variables, local development paths, and bundled sidecars when resolving offline tools. Supported overrides are `FFMPEG_BIN`, `WHISPER_CLI_BIN`, `YT_DLP_BIN`, and `WHISPER_MODEL_PATH`.

## Offline Transcription Flow

```text
Import audio
  → convert to 16 kHz mono WAV with ffmpeg
  → transcribe locally with whisper-cli
  → generate SRT
  → load subtitles into the player
```

## Build and Release

```bash
npm run build
cargo check --manifest-path src-tauri/Cargo.toml
```

GitHub Actions can build Apple Silicon, Intel macOS, and Windows x64 packages from version tags. Platform signing is optional and configured through repository secrets. See [`docs/sidecar-release.md`](./docs/sidecar-release.md) for the sidecar release process.

## Security and Privacy

Transcription runs locally. Audio and subtitle content do not need to be uploaded to a remote service.
