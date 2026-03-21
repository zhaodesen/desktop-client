import { invoke } from "@tauri-apps/api/core";
import { emitTo } from "@tauri-apps/api/event";
import {
  ASR_COMPLETED_EVENT,
  ASR_FAILED_EVENT,
  ASR_PROGRESS_EVENT,
  ASR_STARTED_EVENT,
  IMPORT_PROGRESS_EVENT,
  MODEL_DOWNLOAD_COMPLETED_EVENT,
  MODEL_DOWNLOAD_FAILED_EVENT,
  MODEL_DOWNLOAD_PROGRESS_EVENT,
  MODEL_DOWNLOAD_STARTED_EVENT,
  OVERLAY_CLEAR_EVENT,
  OVERLAY_RENDER_EVENT,
  OVERLAY_STYLE_EVENT,
} from "./events";
import type {
  AllModelsStatus,
  AsrCompletedPayload,
  CancelAsrJobOutput,
  AsrFailedPayload,
  AsrProgressPayload,
  AsrStartedPayload,
  AppSettings,
  CleanupResult,
  CommandResult,
  DefaultModelStatus,
  DownloadModelOutput,
  ImportMediaProgressPayload,
  LibraryState,
  MediaItem,
  ModelDownloadCompletedPayload,
  ModelDownloadFailedPayload,
  ModelDownloadProgressPayload,
  ModelDownloadStartedPayload,
  ModelInfo,
  ModelStatus,
  OverlayRenderPayload,
  OverlayWindowState,
  PlaybackHistoryItem,
  StartAsrJobInput,
  StartAsrJobOutput,
  SubtitleCue,
  SubtitleDocument,
} from "./types";
import { listen } from "@tauri-apps/api/event";

async function callCommand<T>(
  command: string,
  args: Record<string, unknown> = {},
): Promise<T> {
  const result = await invoke<CommandResult<T>>(command, args);

  if (!result.ok || result.data === undefined) {
    const code = result.error?.code;
    const message = result.error?.message ?? `调用 ${command} 失败`;
    throw new Error(code ? `[${code}] ${message}` : message);
  }

  return result.data;
}

export const backend = {
  getSettings() {
    return callCommand<AppSettings>("get_settings");
  },
  updateSettings(settings: AppSettings) {
    return callCommand<AppSettings>("update_settings", { settings });
  },
  showOverlay() {
    return callCommand<OverlayWindowState>("show_overlay");
  },
  hideOverlay() {
    return callCommand<OverlayWindowState>("hide_overlay");
  },
  toggleOverlay() {
    return callCommand<OverlayWindowState>("toggle_overlay");
  },
  startAsrJob(input: StartAsrJobInput) {
    return callCommand<StartAsrJobOutput>("start_asr_job", {
      audioPath: input.audioPath,
    });
  },
  cancelAsrJob() {
    return callCommand<CancelAsrJobOutput>("cancel_asr_job");
  },
  getDefaultModelStatus() {
    return callCommand<DefaultModelStatus>("get_default_model_status");
  },
  downloadDefaultModel() {
    return callCommand<DownloadModelOutput>("download_default_model");
  },
  getAvailableModels() {
    return callCommand<ModelInfo[]>("get_available_models");
  },
  getAllModelsStatus() {
    return callCommand<AllModelsStatus>("get_all_models_status");
  },
  getModelStatus(modelId: string) {
    return callCommand<ModelStatus>("get_model_status", { modelId });
  },
  downloadModel(modelId: string) {
    return callCommand<DownloadModelOutput>("download_model", { modelId });
  },
  deleteModel(modelId: string) {
    return callCommand<CleanupResult>("delete_model", { modelId });
  },
  getLibraryState() {
    return callCommand<LibraryState>("get_library_state");
  },
  importMedia(sourcePath: string) {
    return callCommand<MediaItem>("import_media", { sourcePath });
  },
  deleteMedia(mediaId: string) {
    return callCommand<boolean>("delete_media", { mediaId });
  },
  updateMediaSubtitle(mediaId: string, subtitlePath: string) {
    return callCommand<MediaItem>("update_media_subtitle", {
      mediaId,
      subtitlePath,
    });
  },
  getSubtitleDocument(mediaId: string) {
    return callCommand<SubtitleDocument>("get_subtitle_document", { mediaId });
  },
  saveSubtitleDocument(mediaId: string, cues: SubtitleCue[]) {
    return callCommand<SubtitleDocument>("save_subtitle_document", { mediaId, cues });
  },
  translateMediaSubtitle(mediaId: string) {
    return callCommand<SubtitleDocument>("translate_media_subtitle", { mediaId });
  },
  recordPlayback(mediaId: string) {
    return callCommand<PlaybackHistoryItem>("record_playback", { mediaId });
  },
  clearSubtitles() {
    return callCommand<CleanupResult>("clear_subtitles");
  },
  clearAudioCache() {
    return callCommand<CleanupResult>("clear_audio_cache");
  },
  clearMediaLibrary() {
    return callCommand<CleanupResult>("clear_media_library");
  },
  deleteDefaultModel() {
    return callCommand<CleanupResult>("delete_default_model");
  },
  resetAppData() {
    return callCommand<CleanupResult>("reset_app_data");
  },
};

export const overlayBridge = {
  render(payload: OverlayRenderPayload) {
    return emitTo("overlay", OVERLAY_RENDER_EVENT, payload);
  },
  clear() {
    return emitTo("overlay", OVERLAY_CLEAR_EVENT);
  },
  updateStyle(settings: AppSettings["overlay"]) {
    return emitTo("overlay", OVERLAY_STYLE_EVENT, settings);
  },
};

export const asrEvents = {
  onStarted(handler: (payload: AsrStartedPayload) => void) {
    return listen<AsrStartedPayload>(ASR_STARTED_EVENT, ({ payload }) => {
      handler(payload);
    });
  },
  onProgress(handler: (payload: AsrProgressPayload) => void) {
    return listen<AsrProgressPayload>(ASR_PROGRESS_EVENT, ({ payload }) => {
      handler(payload);
    });
  },
  onCompleted(handler: (payload: AsrCompletedPayload) => void) {
    return listen<AsrCompletedPayload>(ASR_COMPLETED_EVENT, ({ payload }) => {
      handler(payload);
    });
  },
  onFailed(handler: (payload: AsrFailedPayload) => void) {
    return listen<AsrFailedPayload>(ASR_FAILED_EVENT, ({ payload }) => {
      handler(payload);
    });
  },
};

export const importEvents = {
  onProgress(handler: (payload: ImportMediaProgressPayload) => void) {
    return listen<ImportMediaProgressPayload>(IMPORT_PROGRESS_EVENT, ({ payload }) => {
      handler(payload);
    });
  },
};

export const modelEvents = {
  onStarted(handler: (payload: ModelDownloadStartedPayload) => void) {
    return listen<ModelDownloadStartedPayload>(
      MODEL_DOWNLOAD_STARTED_EVENT,
      ({ payload }) => {
        handler(payload);
      },
    );
  },
  onProgress(handler: (payload: ModelDownloadProgressPayload) => void) {
    return listen<ModelDownloadProgressPayload>(
      MODEL_DOWNLOAD_PROGRESS_EVENT,
      ({ payload }) => {
        handler(payload);
      },
    );
  },
  onCompleted(handler: (payload: ModelDownloadCompletedPayload) => void) {
    return listen<ModelDownloadCompletedPayload>(
      MODEL_DOWNLOAD_COMPLETED_EVENT,
      ({ payload }) => {
        handler(payload);
      },
    );
  },
  onFailed(handler: (payload: ModelDownloadFailedPayload) => void) {
    return listen<ModelDownloadFailedPayload>(
      MODEL_DOWNLOAD_FAILED_EVENT,
      ({ payload }) => {
        handler(payload);
      },
    );
  },
};
