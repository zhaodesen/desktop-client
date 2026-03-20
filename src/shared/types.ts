export type SubtitleCue = {
  id: number;
  startMs: number;
  endMs: number;
  text: string;
  secondaryText?: string;
};

export type SubtitleDocument = {
  mediaId: string;
  title: string;
  subtitlePath: string;
  cues: SubtitleCue[];
};

export type MediaSourceKind = "audio" | "video";

export type MediaItem = {
  id: string;
  title: string;
  originalFileName: string;
  sourceKind: MediaSourceKind;
  audioPath: string;
  subtitlePath?: string;
  importedAt: number;
};

export type PlaybackHistoryItem = {
  mediaId: string;
  title: string;
  audioPath: string;
  subtitlePath?: string;
  playedAt: number;
  playCount: number;
};

export type LibraryState = {
  mediaItems: MediaItem[];
  playbackHistory: PlaybackHistoryItem[];
};

export type SubtitleContext = {
  previous?: SubtitleCue;
  current?: SubtitleCue;
  next?: SubtitleCue;
};

export type OverlayPosition = "top" | "bottom";

export type OverlaySettings = {
  fontSize: number;
  opacity: number;
  color: string;
  strokeColor: string;
  secondaryColor: string;
  secondaryStrokeColor: string;
  position: OverlayPosition;
};

export type PlaylistMode = "single" | "sequential";

export type AppSettings = {
  playbackRate: number;
  overlayVisible: boolean;
  overlay: OverlaySettings;
  playlistMode: PlaylistMode;
  selectedModel: string;
};

export type CommandError = {
  code: string;
  message: string;
};

export type CommandResult<T> = {
  ok: boolean;
  data?: T;
  error?: CommandError;
};

export type OverlayWindowState = {
  visible: boolean;
};

export type PlaybackSnapshot = {
  playing: boolean;
  currentTimeMs: number;
  durationMs: number;
  rate: number;
};

export type OverlayRenderPayload = {
  fileLabel?: string;
  previous?: SubtitleCue;
  current?: SubtitleCue;
  next?: SubtitleCue;
  playback: PlaybackSnapshot;
};

export type StartAsrJobInput = {
  audioPath: string;
};

export type StartAsrJobOutput = {
  jobId: string;
};

export type AsrStartedPayload = {
  jobId: string;
  audioPath: string;
};

export type AsrProgressPayload = {
  jobId: string;
  stage: "preparing" | "recognizing" | "writing";
  message: string;
};

export type AsrCompletedPayload = {
  jobId: string;
  subtitlePath: string;
  wavPath: string;
  modelPath: string;
};

export type AsrFailedPayload = {
  jobId: string;
  code: string;
  message: string;
};

export type ModelInfo = {
  id: string;
  label: string;
  description: string;
  fileName: string;
  downloadUrl: string;
  sizeMb: number;
};

export type ModelStatus = {
  modelId: string;
  installed: boolean;
  path?: string;
  source: string;
  sizeBytes?: number;
  downloadUrl: string;
};

export type AllModelsStatus = {
  models: ModelStatus[];
};

/** @deprecated Use ModelStatus instead */
export type DefaultModelStatus = ModelStatus;

export type DownloadModelOutput = {
  jobId: string;
};

export type CleanupResult = {
  deletedFiles: number;
  deletedDirs: number;
};

export type ImportMediaInput = {
  sourcePath: string;
};

export type UpdateMediaSubtitleInput = {
  mediaId: string;
  subtitlePath: string;
};

export type DeleteMediaInput = {
  mediaId: string;
};

export type RecordPlaybackInput = {
  mediaId: string;
};

export type ImportProgressStage =
  | "importing"
  | "preparing"
  | "recognizing"
  | "translating"
  | "done";

export type ImportProgress = {
  active: boolean;
  stage: ImportProgressStage;
  message: string;
  percent: number;
};

export type ModelDownloadStartedPayload = {
  jobId: string;
  modelId: string;
};

export type ModelDownloadProgressPayload = {
  jobId: string;
  message: string;
};

export type ModelDownloadCompletedPayload = {
  jobId: string;
  status: ModelStatus;
};

export type ModelDownloadFailedPayload = {
  jobId: string;
  code: string;
  message: string;
};
