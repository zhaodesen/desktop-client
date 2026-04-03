<script lang="ts">
  import { onMount } from "svelte";
  import { fade } from "svelte/transition";
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { emitTo, listen } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { open } from "@tauri-apps/plugin-dialog";
  import aboutAvatarAsset from "./assets/about-avatar.jpg";
  import { OVERLAY_CLOSE_EVENT, OVERLAY_LOCK_EVENT } from "./shared/events";
  import {
    appEvents,
    asrEvents,
    backend,
    importEvents,
    modelEvents,
    overlayBridge,
    translationModelEvents,
  } from "./shared/tauri";
  import type {
    AppSettings,
    CleanupResult,
    ImportProgress,
    LibraryState,
    MediaItem,
    ModelInfo,
    ModelStatus,
    OverlaySettings,
    PlaybackClockAnchor,
    PlaybackState,
    PlaybackSnapshot,
    PlaylistMode,
    ShutdownTaskSummary,
    SubtitleCue,
    SubtitleDocument,
    ThemeMode,
    TranslationModelInfo,
    TranslationModelStatus,
  } from "./shared/types";
  import { PlayerController } from "./main/player-controller";
  import { buildDisplayCue, createPlaybackClockAnchor } from "./main/lyric-timing";
  import { parseSubtitleText } from "./main/subtitle-parser";
  import { SubtitleEngine } from "./main/subtitle-engine";
  import { formatDuration } from "./shared/utils";

  import Sidebar from "./lib/Sidebar.svelte";
  import ImportPage from "./lib/ImportPage.svelte";
  import ResourceListPage from "./lib/ResourceListPage.svelte";
  import PlayerPage from "./lib/PlayerPage.svelte";
  import PlayerBar from "./lib/PlayerBar.svelte";
  import SettingsPage from "./lib/SettingsPage.svelte";
  import AboutPage from "./lib/AboutPage.svelte";
  import SubtitleEditor from "./lib/SubtitleEditor.svelte";
  import ConfirmDialog from "./lib/ConfirmDialog.svelte";
  import FirstRunOnboarding from "./lib/FirstRunOnboarding.svelte";
  import "./styles.css";

  /* ── Constants ─────────────────────────────────────────── */

  const ONBOARDING_MODEL_GUIDES: Record<string, { pros: string[]; cons: string[]; recommended?: boolean }> = {
    tiny: {
      pros: ["下载最快", "占用最小", "老机器也能跑"],
      cons: ["准确率最低", "嘈杂音频容易漏词"],
    },
    base: {
      pros: ["速度和效果平衡", "对大多数日常素材够用", "安装体积适中"],
      cons: ["复杂口音和多人对话下仍有误差"],
      recommended: true,
    },
    small: {
      pros: ["准确率明显更好", "适合中长视频", "对口音更稳"],
      cons: ["下载和推理都更慢", "对磁盘与内存要求更高"],
    },
    medium: {
      pros: ["高准确率", "适合课程、访谈等长音频", "中文字幕校对工作更少"],
      cons: ["文件很大", "低配设备耗时明显"],
    },
    "large-v3-turbo": {
      pros: ["整体效果最好", "对复杂语音场景更稳", "适合高质量转写"],
      cons: ["下载最大", "启动和识别耗时最长"],
    },
  };

  const PLAYBACK_STATE_SAVE_DEBOUNCE_MS = 250;
  const PLAYBACK_STATE_PROGRESS_THRESHOLD_MS = 1000;
  const DEFAULT_VOLUME = 1;
  const BOOTSTRAP_SETTINGS: AppSettings = {
    playbackRate: 1,
    volume: DEFAULT_VOLUME,
    overlayVisible: true,
    overlay: {
      fontSize: 34,
      opacity: 1,
      color: "#ffffff",
      strokeColor: "#000000",
      secondaryColor: "#ffffff",
      secondaryStrokeColor: "#000000",
      position: "bottom",
    },
    playlistMode: "sequential",
    subtitleDisplayMode: "bilingual",
    shortcuts: {
      playPause: "Space",
      previousTrack: "Comma",
      nextTrack: "Period",
      toggleOverlay: "KeyO",
      volumeUp: "Equal",
      volumeDown: "Minus",
      showTranslation: "Digit1",
      showOriginal: "Digit2",
      showBilingual: "Digit3",
    },
    selectedModel: "base",
    hasCompletedOnboarding: false,
    hasSeenMainTour: false,
    themeMode: "dark",
  };
  const ABOUT_AVATAR_SRC = resolveAboutAvatarSrc();
  const FORCE_ONBOARDING = parseBooleanEnv(import.meta.env.VITE_FORCE_ONBOARDING);
  const SHOW_ONBOARDING_PREVIEW_ENTRY = import.meta.env.DEV;

  function parseBooleanEnv(value: unknown): boolean {
    if (typeof value !== "string") return false;
    return ["1", "true", "yes", "on"].includes(value.trim().toLowerCase());
  }

  function resolveAboutAvatarSrc(): string {
    const envPath = typeof import.meta.env.VITE_ABOUT_AVATAR_PATH === "string"
      ? import.meta.env.VITE_ABOUT_AVATAR_PATH.trim()
      : "";

    if (!envPath) return aboutAvatarAsset;
    if (/^(asset:|https?:|data:|blob:)/i.test(envPath)) return envPath;
    return convertFileSrc(envPath);
  }

  /* ── State ─────────────────────────────────────────────── */

  let settings = $state<AppSettings>({ ...BOOTSTRAP_SETTINGS, overlay: { ...BOOTSTRAP_SETTINGS.overlay }, shortcuts: { ...BOOTSTRAP_SETTINGS.shortcuts } });
  let settingsReady = $state(false);
  let libraryState = $state<LibraryState>({ mediaItems: [], playbackHistory: [] });
  let activePage = $state("import");
  let lastMainPage = $state<"import" | "resources" | "playlist" | "settings" | "about">("import");
  let aboutAvatarSrc = $state("");

  let currentMediaId = $state<string | undefined>(undefined);
  let pendingPlaylistMediaId = $state<string | undefined>(undefined);
  let activeAsrJobId = $state<string | undefined>(undefined);
  let activeModelDownloadJobId = $state<string | undefined>(undefined);
  let downloadingModelId = $state<string | undefined>(undefined);
  let modelDownloadPercent = $state(0);
  let isDownloadPaused = $state(false);
  let modelDownloadSuccessLabel = $state<string | undefined>(undefined);
  let hasTriedAutoResumeModelDownload = false;
  let modelDownloadSuccessTimer: ReturnType<typeof setTimeout> | undefined;
  let activeTranslationModelDownloadJobId = $state<string | undefined>(undefined);
  let translationModelDownloadPercent = $state(0);
  let isTranslationModelDownloadPaused = $state(false);
  let hasTriedAutoResumeTranslationModelDownload = false;
  let pendingSubtitleMediaId = $state<string | undefined>(undefined);
  let overlayLocked = $state(false);

  // Import UI state: progress tracks the entire pipeline
  const IMPORT_IDLE: ImportProgress = { active: false, stage: "done", message: "", percent: 0 };
  let importProgress = $state<ImportProgress>({ ...IMPORT_IDLE });
  let importError = $state<string | undefined>(undefined);
  let importSuccessName = $state<string | undefined>(undefined);
  let importSuccessKind = $state<"bilingual" | "original" | "translation-failed">("bilingual");
  let showImportSuccess = $state(false);
  let importSuccessTimer: ReturnType<typeof setTimeout> | undefined;
  let translationProgressDriftTimer: ReturnType<typeof setInterval> | undefined;
  let isCancellingAsr = $state(false);

  let availableModels = $state<ModelInfo[]>([]);
  let modelsStatusMap = $state<Map<string, ModelStatus>>(new Map());
  let translationModelInfo = $state<TranslationModelInfo | undefined>(undefined);
  let translationModelStatus = $state<TranslationModelStatus | undefined>(undefined);
  let activeSubtitleDocument = $state<SubtitleDocument | undefined>(undefined);
  let subtitleEditorNotice = $state<string | undefined>(undefined);
  let subtitleEditorSaving = $state(false);
  let retryAsrProgress = $state<{ mediaId: string; percent: number; message: string } | undefined>(undefined);
  let retryAsrCompletedMediaId = $state<string | undefined>(undefined);
  let retryAsrCompletedMessage = $state<string | undefined>(undefined);
  let retryAsrNoticeTimer: ReturnType<typeof setTimeout> | undefined;
  let retryAsrProgressDriftTimer: ReturnType<typeof setInterval> | undefined;
  let activeImportSource = $state<"local" | "online" | undefined>(undefined);
  let showFirstRunOnboarding = $state(false);
  let onboardingStep = $state<"select-model" | "downloading" | "ready">("select-model");
  let onboardingSelectedModelId = $state<string | undefined>(undefined);
  let onboardingDownloadPercent = $state(0);
  let onboardingDownloadMessage = $state("正在准备模型下载…");
  let onboardingError = $state<string | undefined>(undefined);

  // Player state (published by PlayerController)
  let snap = $state<PlaybackSnapshot>({ playing: false, currentTimeMs: 0, durationMs: 0, rate: 1, volume: 1 });
  let playbackAnchor = $state<PlaybackClockAnchor>({
    mediaTimeMs: 0,
    wallTimeMs: Date.now(),
    durationMs: 0,
    rate: 1,
    playing: false,
  });
  let hasMedia = $state(false);
  let audioFileLabel = $state("未选择素材");
  let subtitleFileLabel = $state("未生成字幕");
  let cueTiming = $state("--:-- ~ --:--");
  let currentText = $state("等待播放");
  let currentSecondaryText = $state("");
  let subtitleCues = $state<SubtitleCue[]>([]);
  let lastAudibleVolume = $state(DEFAULT_VOLUME);
  let lastSavedPlaybackState: PlaybackState | undefined;
  let pendingPlaybackState: PlaybackState | undefined;
  let playbackStateSaveTimer: ReturnType<typeof setTimeout> | undefined;
  let playbackStateSavePromise: Promise<void> | null = null;
  let restoringPlaybackState = false;
  let playbackStateHydrated = false;
  let useWindowsCustomFrame = $state(false);
  let windowMaximized = $state(false);

  // Status bar
  let statusText = $state("导入媒体后自动生成双语字幕");
  type StatusTone = "neutral" | "success" | "warning";
  let statusTone = $state<StatusTone>("neutral");
  let statusBadgeLabel = $state("就绪");

  // Model status labels
  let modelStatusLabel = $state("正在检查模型状态…");
  let modelPathLabel = $state("模型路径加载中");
  let translationModelStatusLabel = $state("正在检查翻译模型状态…");
  let translationModelPathLabel = $state("翻译模型路径加载中");

  // ASR 进度更新 requestAnimationFrame 节流：
  // 即使 Rust 侧已节流到 1 秒 1 次，Svelte 的同步 DOM diff 仍可能导致掉帧。
  // 用 rAF 将状态更新推迟到下一个渲染帧，避免在高频事件回调中直接触发重排。
  let pendingProgressUpdate: ImportProgress | null = null;
  let progressRafId = 0;
  let mediaLoadRequestId = 0;
  let playlistPlayPromise: Promise<void> | null = null;
  let playlistPlayTargetId: string | undefined;
  type ImportSource = "local" | "online";
  type ImportBackendStage = "downloading" | "copying" | "extracting" | "registering";

  function scheduleProgressUpdate(next: ImportProgress) {
    const baseline = Math.max(
      importProgress.active ? importProgress.percent : 0,
      pendingProgressUpdate?.active ? pendingProgressUpdate.percent : 0,
    );
    pendingProgressUpdate = next.active
      ? { ...next, percent: Math.max(next.percent, baseline) }
      : next;
    if (!progressRafId) {
      progressRafId = requestAnimationFrame(() => {
        progressRafId = 0;
        if (pendingProgressUpdate) {
          importProgress = pendingProgressUpdate;
          pendingProgressUpdate = null;
        }
      });
    }
  }

  function resetScheduledProgressUpdate() {
    pendingProgressUpdate = null;
    if (progressRafId) {
      cancelAnimationFrame(progressRafId);
      progressRafId = 0;
    }
  }

  // Internal services (created in onMount)
  let player: PlayerController;
  let subtitleEngine: SubtitleEngine;
  let audioEl: HTMLAudioElement;
  const tauriWindow = getCurrentWindow();

  // ConfirmDialog ref
  let confirmDialog: ConfirmDialog;

  aboutAvatarSrc = ABOUT_AVATAR_SRC;

  /* ── Theme ─────────────────────────────────────────────── */

  function applyTheme(mode: ThemeMode) {
    let resolved: "dark" | "light" = mode === "system"
      ? (window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light")
      : mode;
    document.documentElement.setAttribute("data-theme", resolved);
  }

  $effect(() => {
    if (settingsReady) applyTheme(settings.themeMode);
  });

  /* ── Helpers ────────────────────────────────────────────── */

  function setStatus(text: string, tone: StatusTone = "neutral") {
    statusText = text;
    statusTone = tone;
    statusBadgeLabel = activeAsrJobId || activeModelDownloadJobId
      ? "运行中"
      : tone === "success" ? "完成" : tone === "warning" ? "注意" : "就绪";
  }

  function formatError(err: unknown): string {
    if (err instanceof Error) return err.message;
    if (typeof err === "string") return err;
    return "未知错误";
  }

  function describeMediaError(error: MediaError | null): string {
    if (!error) return "none";

    const codeMap: Record<number, string> = {
      1: "MEDIA_ERR_ABORTED",
      2: "MEDIA_ERR_NETWORK",
      3: "MEDIA_ERR_DECODE",
      4: "MEDIA_ERR_SRC_NOT_SUPPORTED",
    };

    return `${codeMap[error.code] ?? `UNKNOWN(${error.code})`}: ${error.message || "no message"}`;
  }

  async function handleTogglePlayback(source: string) {
    if (!hasMedia) return;

    try {
      await player.togglePlayback();
    } catch (err) {
      console.error(err);
      setStatus(formatError(err), "warning");
    }
  }

  function fmtCleanup(r: CleanupResult): string {
    return `已删除 ${r.deletedFiles} 个文件，${r.deletedDirs} 个目录`;
  }

  function waitForNextPaint() {
    return new Promise<void>((resolve) => {
      requestAnimationFrame(() => resolve());
    });
  }

  function stopTranslationProgressDrift() {
    clearInterval(translationProgressDriftTimer);
    translationProgressDriftTimer = undefined;
  }

  function startTranslationProgressDrift() {
    stopTranslationProgressDrift();
    translationProgressDriftTimer = setInterval(() => {
      if (!importProgress.active || importProgress.stage !== "translating") {
        stopTranslationProgressDrift();
        return;
      }

      if (importProgress.percent >= 97) return;

      scheduleProgressUpdate({
        ...importProgress,
        percent: Math.min(importProgress.percent + 1, 97),
      });
    }, 900);
  }

  function clampProgressPercent(percent: number | null | undefined) {
    return Math.max(0, Math.min(100, percent ?? 0));
  }

  function mapProgressRange(start: number, end: number, percent: number | null | undefined, fallback: number) {
    if (percent == null) return fallback;
    const normalized = clampProgressPercent(percent);
    return start + (normalized / 100) * (end - start);
  }

  function getImportProgressSource(source = activeImportSource): ImportSource {
    return source === "online" ? "online" : "local";
  }

  function getImportSettledPercent(source = activeImportSource) {
    return getImportProgressSource(source) === "online" ? 60 : 24;
  }

  function getPreparingOverallPercent(source = activeImportSource) {
    return getImportProgressSource(source) === "online" ? 66 : 30;
  }

  function getRecognizingOverallPercent(percent: number | null | undefined, source = activeImportSource) {
    return getImportProgressSource(source) === "online"
      ? mapProgressRange(72, 88, percent, 72)
      : mapProgressRange(36, 88, percent, 36);
  }

  function getWritingOverallPercent() {
    return 90;
  }

  function getTranslatingOverallPercent() {
    return 92;
  }

  function getImportStageOverallPercent(
    stage: ImportBackendStage,
    percent: number | null | undefined,
    source = activeImportSource,
  ) {
    if (stage === "downloading") {
      return getImportProgressSource(source) === "online"
        ? mapProgressRange(4, 52, percent, 10)
        : 0;
    }

    if (getImportProgressSource(source) === "online") {
      if (stage === "copying") return mapProgressRange(52, 58, percent, 54);
      if (stage === "extracting") return mapProgressRange(52, 58, percent, 55);
      return percent != null ? 60 : 58;
    }

    if (stage === "copying") return mapProgressRange(6, 16, percent, 10);
    if (stage === "extracting") return mapProgressRange(16, 24, percent, 20);
    return percent != null ? 24 : 22;
  }

  function clearRetryAsrCompletionNotice() {
    clearTimeout(retryAsrNoticeTimer);
    retryAsrNoticeTimer = undefined;
    retryAsrCompletedMediaId = undefined;
    retryAsrCompletedMessage = undefined;
  }

  function stopRetryAsrProgressDrift() {
    clearInterval(retryAsrProgressDriftTimer);
    retryAsrProgressDriftTimer = undefined;
  }

  function startRetryAsrProgressDrift(mediaId: string) {
    stopRetryAsrProgressDrift();
    retryAsrProgressDriftTimer = setInterval(() => {
      if (!retryAsrProgress || retryAsrProgress.mediaId !== mediaId) {
        stopRetryAsrProgressDrift();
        return;
      }
      const currentPercent = retryAsrProgress.percent;
      if (currentPercent >= 88) return;
      const nextStep = currentPercent < 40 ? 3 : currentPercent < 70 ? 2 : 1;
      retryAsrProgress = {
        ...retryAsrProgress,
        percent: Math.min(currentPercent + nextStep, 88),
      };
    }, 900);
  }

  function updateRetryAsrProgress(mediaId: string, percent: number, message: string) {
    const previousPercent = retryAsrProgress?.mediaId === mediaId ? retryAsrProgress.percent : 0;
    retryAsrProgress = {
      mediaId,
      percent: Math.max(previousPercent, Math.max(0, Math.min(100, Math.round(percent)))),
      message,
    };
  }

  function finishRetryAsrNotice(mediaId: string, message = "重新识别完成") {
    stopRetryAsrProgressDrift();
    retryAsrProgress = undefined;
    retryAsrCompletedMediaId = mediaId;
    retryAsrCompletedMessage = message;
    clearTimeout(retryAsrNoticeTimer);
    retryAsrNoticeTimer = setTimeout(() => {
      if (retryAsrCompletedMediaId === mediaId) {
        retryAsrCompletedMediaId = undefined;
        retryAsrCompletedMessage = undefined;
      }
    }, 2200);
  }

  function getCurrentMedia(): MediaItem | undefined {
    return libraryState.mediaItems.find((i) => i.id === currentMediaId);
  }

  function getSavedPlaybackState(): PlaybackState | undefined {
    return settings?.playbackState
      ? normalizePlaybackState(settings.playbackState)
      : undefined;
  }

  function normalizePlaybackState(playbackState: PlaybackState): PlaybackState {
    return {
      mediaId: playbackState.mediaId,
      currentTimeMs: Math.max(0, Math.round(playbackState.currentTimeMs)),
      wasPlaying: playbackState.wasPlaying,
    };
  }

  function isSamePlaybackState(
    left: PlaybackState | undefined,
    right: PlaybackState | undefined,
    toleranceMs = 0,
  ) {
    if (!left && !right) return true;
    if (!left || !right) return false;
    return left.mediaId === right.mediaId
      && left.wasPlaying === right.wasPlaying
      && Math.abs(left.currentTimeMs - right.currentTimeMs) <= toleranceMs;
  }

  function getPlaybackStateFromSnapshot(snapshot = player.getSnapshot()): PlaybackState | undefined {
    if (!currentMediaId || !player.hasMedia()) return undefined;
    return normalizePlaybackState({
      mediaId: currentMediaId,
      currentTimeMs: snapshot.currentTimeMs,
      wasPlaying: snapshot.playing,
    });
  }

  function getPlaybackStateForPersist(snapshot = player.getSnapshot()): PlaybackState | undefined {
    const currentPlaybackState = getPlaybackStateFromSnapshot(snapshot);
    if (currentPlaybackState) return currentPlaybackState;
    if (!playbackStateHydrated) return getSavedPlaybackState();
    return undefined;
  }

  async function flushPlaybackState() {
    if (playbackStateSaveTimer) {
      clearTimeout(playbackStateSaveTimer);
      playbackStateSaveTimer = undefined;
    }

    if (playbackStateSavePromise) {
      await playbackStateSavePromise;
      if (pendingPlaybackState === undefined) return;
    }

    const nextPlaybackState = pendingPlaybackState;
    pendingPlaybackState = undefined;

    playbackStateSavePromise = (async () => {
      try {
        const saved = await backend.updatePlaybackState(nextPlaybackState);
        const normalized = saved ? normalizePlaybackState(saved) : undefined;
        if (settings) settings = { ...settings, playbackState: normalized };
        lastSavedPlaybackState = normalized;
      } catch (err) {
        console.error(err);
      } finally {
        playbackStateSavePromise = null;
        if (pendingPlaybackState !== undefined) {
          void flushPlaybackState();
        }
      }
    })();

    await playbackStateSavePromise;
  }

  function queuePlaybackStatePersist(playbackState: PlaybackState | undefined, immediate = false) {
    if (!playbackStateHydrated && !immediate) return;

    const normalized = playbackState ? normalizePlaybackState(playbackState) : undefined;
    const baseline = pendingPlaybackState ?? lastSavedPlaybackState ?? settings?.playbackState;
    const toleranceMs = immediate ? 0 : PLAYBACK_STATE_PROGRESS_THRESHOLD_MS;
    if (isSamePlaybackState(baseline, normalized, toleranceMs)) return;

    pendingPlaybackState = normalized;
    if (settings) settings = { ...settings, playbackState: normalized };

    if (playbackStateSaveTimer) {
      clearTimeout(playbackStateSaveTimer);
      playbackStateSaveTimer = undefined;
    }

    if (immediate) {
      void flushPlaybackState();
      return;
    }

    playbackStateSaveTimer = setTimeout(() => {
      playbackStateSaveTimer = undefined;
      void flushPlaybackState();
    }, PLAYBACK_STATE_SAVE_DEBOUNCE_MS);
  }

  async function clearPlaybackPersistenceState() {
    if (playbackStateSaveTimer) {
      clearTimeout(playbackStateSaveTimer);
      playbackStateSaveTimer = undefined;
    }

    pendingPlaybackState = undefined;

    if (playbackStateSavePromise) {
      await playbackStateSavePromise;
    }

    lastSavedPlaybackState = undefined;
    pendingPlaybackState = undefined;
    if (settings) settings = { ...settings, playbackState: undefined };
  }

  async function restorePlaybackState() {
    const playbackState = getSavedPlaybackState();
    if (!playbackState?.mediaId) return;

    const media = libraryState.mediaItems.find((item) => item.id === playbackState.mediaId);
    if (!media) {
      queuePlaybackStatePersist(undefined, true);
      return;
    }

    restoringPlaybackState = true;
    try {
      if (playbackState.wasPlaying) {
        setActivePage("playlist");
      }
      const loaded = await loadMediaById(playbackState.mediaId, false);
      if (!loaded) return;

      const snapshotAfterLoad = player.getSnapshot();
      const maxSeekMs = snapshotAfterLoad.durationMs > 0
        ? Math.max(snapshotAfterLoad.durationMs - 250, 0)
        : playbackState.currentTimeMs;
      const targetTimeMs = Math.max(0, Math.min(playbackState.currentTimeMs, maxSeekMs));

      if (targetTimeMs > 0) {
        player.seek(targetTimeMs);
        renderSubtitle(player.getSnapshot());
        await syncOverlay(player.getSnapshot());
      }

      if (playbackState.wasPlaying) {
        try {
          await player.play();
          setStatus("已恢复上次播放", "success");
        } catch (err) {
          console.error(err);
          setStatus("已恢复上次播放位置，请手动继续播放", "warning");
        }
      } else {
        setStatus("已恢复上次播放位置", "success");
      }

      queuePlaybackStatePersist(getPlaybackStateFromSnapshot(), true);
    } finally {
      restoringPlaybackState = false;
    }
  }

  function buildAppExitWarning(summary: ShutdownTaskSummary): string {
    return [
      `当前仍有后台任务正在进行：${summary.tasks.join("、")}。`,
      "确认退出后，应用会先终止这些后台任务并执行清理，然后再关闭。",
      "确定仍要退出吗？",
    ].join(" ");
  }

  async function handleAppCloseRequest(summary?: ShutdownTaskSummary) {
    const closeSummary = summary ?? await backend.getShutdownTaskSummary();
    if (closeSummary.hasActiveTasks) {
      const ok = await confirmDialog.show("退出应用", buildAppExitWarning(closeSummary));
      if (!ok) return false;
    }

    queuePlaybackStatePersist(getPlaybackStateForPersist(), true);
    await flushPlaybackState();

    const result = await backend.shutdownAndExit();
    if (result.cancelledTasks.length > 0) {
      setStatus(`正在关闭后台任务并退出应用：${result.cancelledTasks.join("、")}`, "warning");
    } else {
      setStatus("正在退出应用…", "warning");
    }
    return true;
  }

  function setActivePage(page: string) {
    if (page === "import" || page === "resources" || page === "playlist" || page === "settings" || page === "about") {
      lastMainPage = page;
    }
    activePage = page;
  }

  async function handleOnboardingStart() {
    if (!settingsReady) return;
    setActivePage("import");
    showFirstRunOnboarding = false;
    onboardingError = undefined;
    onboardingStep = "ready";
    settings = { ...settings, hasCompletedOnboarding: true };
    await persistSettings();
  }

  async function handleOnboardingSkip() {
    if (!settingsReady) return;
    const ok = await confirmDialog.show(
      "跳过模型选择",
      "如果不选择模型，将无法进行音视频识别。你仍可以稍后在设置页面下载模型。",
    );
    if (!ok) return;

    showFirstRunOnboarding = false;
    onboardingError = undefined;
    onboardingStep = "select-model";
    settings = { ...settings, hasCompletedOnboarding: true };
    await persistSettings();
    setActivePage("import");
    setStatus("已跳过模型选择，可稍后在设置页面下载模型", "warning");
  }

  function openOnboardingPreview() {
    showFirstRunOnboarding = true;
    onboardingStep = "select-model";
    onboardingSelectedModelId = undefined;
    onboardingDownloadPercent = 0;
    onboardingDownloadMessage = "正在准备模型下载…";
    onboardingError = undefined;
    setStatus("已打开首次引导预览", "success");
  }

  /* ── Persist settings ──────────────────────────────────── */

  function buildSettingsForPersist(nextSettings: AppSettings = settings): AppSettings {
    if (!nextSettings) {
      throw new Error("设置尚未加载完成");
    }
    if (nextSettings.playbackState) return nextSettings;
    const playbackState = pendingPlaybackState ?? lastSavedPlaybackState ?? getSavedPlaybackState();
    return playbackState
      ? { ...nextSettings, playbackState }
      : nextSettings;
  }

  async function persistSettings() {
    settings = await backend.updateSettings(buildSettingsForPersist());
    await overlayBridge.updateStyle(settings.overlay);
  }

  async function initializeOverlayAfterStartup() {
    if (!settings.overlayVisible) return;

    try {
      await waitForNextPaint();
      await backend.showOverlay();
      await overlayBridge.updateStyle(settings.overlay);
      await syncOverlay(player.getSnapshot());
    } catch (err) {
      console.error(err);
      setStatus("初始化悬浮窗失败", "warning");
    }
  }

  async function hydrateInitialData() {
    try {
      await waitForNextPaint();
      await refreshLibrary();
    } catch (err) {
      console.error(err);
      setStatus("读取素材库失败", "warning");
    }

    try {
      await restorePlaybackState();
    } catch (err) {
      console.error(err);
      setStatus("恢复上次播放失败", "warning");
    } finally {
      playbackStateHydrated = true;
    }

    try {
      const [models, allStatus, fetchedTranslationInfo, fetchedTranslationStatus] = await Promise.all([
        backend.getAvailableModels(),
        backend.getAllModelsStatus(),
        backend.getTranslationModelInfo(),
        backend.getTranslationModelStatus(),
      ]);
      availableModels = models;
      const newMap = new Map<string, ModelStatus>();
      for (const s of allStatus.models) newMap.set(s.modelId, s);
      modelsStatusMap = newMap;
      translationModelInfo = fetchedTranslationInfo;
      translationModelStatus = fetchedTranslationStatus;
      refreshModelLabels();
      refreshTranslationModelLabels();
      void maybeResumeModelDownload();
      void maybeResumeTranslationModelDownload();
    } catch (err) {
      console.error(err);
      modelStatusLabel = "模型状态读取失败";
      translationModelStatusLabel = "翻译模型状态读取失败";
    }
  }

  async function maybeResumeModelDownload() {
    if (hasTriedAutoResumeModelDownload || activeModelDownloadJobId) return;
    hasTriedAutoResumeModelDownload = true;

    try {
      const resumable = await backend.getResumableModelDownload();
      if (!resumable) return;

      const label = availableModels.find((m) => m.id === resumable.modelId)?.label ?? resumable.modelId;
      if (showFirstRunOnboarding) {
        onboardingSelectedModelId = resumable.modelId;
        onboardingStep = "downloading";
        onboardingError = undefined;
        onboardingDownloadPercent = 0;
        onboardingDownloadMessage = "检测到未完成下载，正在继续…";
        await handleSelectModel(resumable.modelId, { silent: true });
      }

      await handleDownloadModel(resumable.modelId, { silent: true, resuming: true });
      setStatus(`检测到未完成下载，正在继续模型 ${label}`, "neutral");
    } catch (err) {
      console.error(err);
    }
  }

  /* ── Model UI helpers ──────────────────────────────────── */

  function refreshModelLabels() {
    if (!settingsReady) return;
    const selected = settings.selectedModel || "base";
    const selectedStatus = modelsStatusMap.get(selected);
    if (selectedStatus) {
      const label = availableModels.find((m) => m.id === selected)?.label ?? selected;
      modelStatusLabel = selectedStatus.installed
        ? `当前选用: ${label} · 已就绪`
        : `当前选用: ${label} · 未安装`;
      modelPathLabel = selectedStatus.path ?? "模型未下载";
    } else {
      modelStatusLabel = "正在检查模型状态…";
      modelPathLabel = "";
    }
  }

  function refreshTranslationModelLabels() {
    if (!translationModelInfo || !translationModelStatus) {
      translationModelStatusLabel = "正在检查翻译模型状态…";
      translationModelPathLabel = "翻译模型路径加载中";
      return;
    }

    translationModelStatusLabel = translationModelStatus.installed
      ? `${translationModelInfo.label} · 已就绪`
      : `${translationModelInfo.label} · 未安装`;
    translationModelPathLabel = translationModelStatus.path ?? "翻译模型未下载";
  }

  /* ── Subtitle helpers ──────────────────────────────────── */

  function isChineseLanguage(code?: string): boolean {
    return code?.trim().toLowerCase().startsWith("zh") ?? false;
  }

  function getDisplayedCue(cue?: SubtitleCue): SubtitleCue | undefined {
    if (!cue) return undefined;
    if (!settingsReady) return cue;
    return buildDisplayCue(cue, settings.subtitleDisplayMode);
  }

  function renderSubtitle(s: PlaybackSnapshot) {
    const ctx = subtitleEngine.getContext(s.currentTimeMs);
    const displayedCue = getDisplayedCue(ctx.current);
    currentText = displayedCue?.text ?? "当前时间点暂无字幕";
    currentSecondaryText = displayedCue?.secondaryText ?? "";
    cueTiming = ctx.current
      ? `${formatDuration(ctx.current.startMs)} ~ ${formatDuration(ctx.current.endMs)}`
      : "--:-- ~ --:--";
  }

  async function syncOverlay(s: PlaybackSnapshot) {
    if (!settingsReady) return;
    if (!settings.overlayVisible) return;

    const media = getCurrentMedia();
    const ctx = subtitleEngine.getContext(s.currentTimeMs);
    await overlayBridge.render({
      fileLabel: media?.title,
      previous: undefined,
      current: ctx.current,
      next: undefined,
      playback: s,
      playbackAnchor,
      subtitleDisplayMode: settings.subtitleDisplayMode,
    });
  }

  async function loadSubtitleFromPath(path: string) {
    const content = await fetch(convertFileSrc(path)).then((r) => r.text());
    const cues = parseSubtitleText(content);
    if (cues.length === 0) throw new Error("未解析出有效字幕");
    subtitleEngine.load(cues);
    subtitleCues = cues;
    subtitleFileLabel = `${path.split(/[\\/]/).pop() ?? "字幕"} · ${cues.length} 句`;
  }

  /* ── Media operations ──────────────────────────────────── */

  async function resetPlaybackUi() {
    subtitleEngine.clear();
    subtitleCues = [];
    currentMediaId = undefined;
    player.pause();
    audioFileLabel = "未选择素材";
    subtitleFileLabel = "未生成字幕";
    currentText = "等待播放";
    currentSecondaryText = "";
    cueTiming = "--:-- ~ --:--";
    await overlayBridge.clear();
    queuePlaybackStatePersist(undefined, true);
  }

  function createMediaLoadRequestId() {
    mediaLoadRequestId += 1;
    return mediaLoadRequestId;
  }

  function isLatestMediaLoadRequest(requestId: number) {
    return requestId === mediaLoadRequestId;
  }

  async function loadMediaById(mediaId: string, record: boolean, requestId = createMediaLoadRequestId()) {
    if (!settingsReady) {
      setStatus("设置尚未加载完成", "warning");
      return false;
    }

    const media = libraryState.mediaItems.find((i) => i.id === mediaId);
    if (!media) {
      if (isLatestMediaLoadRequest(requestId)) {
        setStatus("未找到对应素材", "warning");
      }
      return false;
    }

    const assetUrl = convertFileSrc(media.audioPath);
    await player.loadUrl(assetUrl);
    if (!isLatestMediaLoadRequest(requestId)) return false;

    currentMediaId = media.id;
    player.setPlaybackRate(settings.playbackRate);
    player.setVolume(settings.volume);
    audioFileLabel = media.title;
    subtitleEngine.clear();
    subtitleFileLabel = media.subtitlePath ? "正在加载字幕…" : "未生成字幕";

    if (media.subtitlePath) {
      try {
        await loadSubtitleFromPath(media.subtitlePath);
        if (!isLatestMediaLoadRequest(requestId)) return false;
      } catch (err) {
        console.error(err);
        if (isLatestMediaLoadRequest(requestId)) {
          subtitleFileLabel = "字幕加载失败";
        }
      }
    }

    if (!isLatestMediaLoadRequest(requestId)) return false;
    renderSubtitle(player.getSnapshot());
    await syncOverlay(player.getSnapshot());
    if (!isLatestMediaLoadRequest(requestId)) return false;

    if (record) {
      await backend.recordPlayback(media.id);
      if (!isLatestMediaLoadRequest(requestId)) return false;
      await refreshLibrary();
    }

    if (!restoringPlaybackState) {
      queuePlaybackStatePersist(getPlaybackStateFromSnapshot(), true);
    }

    return isLatestMediaLoadRequest(requestId);
  }

  async function deleteMediaById(mediaId: string) {
    const ok = await confirmDialog.show("删除素材", "确定要删除该素材吗？将同时删除字幕及音频文件，此操作不可逆。");
    if (!ok) return;
    await backend.deleteMedia(mediaId);
    if (currentMediaId === mediaId) await resetPlaybackUi();
    await refreshLibrary();
    setStatus("素材已删除", "success");
  }

  async function refreshLibrary() {
    libraryState = await backend.getLibraryState();
  }

  function applyVolume(volume: number) {
    if (!settingsReady) return;
    const nextVolume = Math.max(0, Math.min(1, volume));
    settings = { ...settings, volume: nextVolume };
    player.setVolume(nextVolume);
  }

  async function toggleMute() {
    if (!settingsReady) return;
    if (settings.volume > 0) {
      applyVolume(0);
      await commitVolume(false);
      setStatus("已静音", "success");
      return;
    }

    const restoredVolume = Math.max(0.05, Math.min(1, lastAudibleVolume || DEFAULT_VOLUME));
    applyVolume(restoredVolume);
    await commitVolume(false);
    setStatus(`已恢复音量 ${Math.round(restoredVolume * 100)}%`, "success");
  }

  async function commitVolume(showFeedback = true) {
    if (!settingsReady) return;
    await persistSettings();
    if (showFeedback) {
      setStatus(`音量已调整为 ${Math.round(settings.volume * 100)}%`, "success");
    }
  }

  async function setSubtitleDisplayMode(mode: AppSettings["subtitleDisplayMode"], showFeedback = true) {
    if (!settingsReady) return;
    if (settings.subtitleDisplayMode === mode) return;
    settings = { ...settings, subtitleDisplayMode: mode };
    await persistSettings();
    renderSubtitle(player.getSnapshot());
    await syncOverlay(player.getSnapshot());
    if (showFeedback) {
      const labelMap: Record<AppSettings["subtitleDisplayMode"], string> = {
        original: "仅显示原文字幕",
        translation: "仅显示中文字幕",
        bilingual: "显示双语字幕",
      };
      setStatus(labelMap[mode], "success");
    }
  }

  $effect(() => {
    if (settings.volume > 0) {
      lastAudibleVolume = settings.volume;
    }
  });

  async function playHistoryDirection(direction: -1 | 1) {
    if (libraryState.playbackHistory.length === 0) return;
    if (!currentMediaId) {
      const first = libraryState.playbackHistory[0];
      await loadMediaById(first.mediaId, true);
      await player.play();
      return;
    }

    const currentIndex = libraryState.playbackHistory.findIndex((item) => item.mediaId === currentMediaId);
    if (currentIndex === -1) return;
    const nextIndex =
      (currentIndex + direction + libraryState.playbackHistory.length) % libraryState.playbackHistory.length;
    const nextItem = libraryState.playbackHistory[nextIndex];
    await loadMediaById(nextItem.mediaId, true);
    await player.play();
  }

  async function removePlaybackItem(mediaId: string) {
    await backend.removePlaybackItem(mediaId);
    await refreshLibrary();
    if (currentMediaId === mediaId && libraryState.playbackHistory.length === 0) {
      await resetPlaybackUi();
    }
    setStatus("已从播放列表移除", "success");
  }

  async function playPlaylistItem(mediaId: string, autoplay = false, record = true) {
    if (playlistPlayPromise && playlistPlayTargetId === mediaId) {
      await playlistPlayPromise;
      return;
    }

    const requestId = createMediaLoadRequestId();
    pendingPlaylistMediaId = mediaId;
    playlistPlayTargetId = mediaId;

    const task = (async () => {
      const loaded = await loadMediaById(mediaId, record, requestId);
      if (autoplay && loaded && isLatestMediaLoadRequest(requestId)) {
        await handleTogglePlayback("playlist-autoplay");
      }
    })();

    const guardedTask = task.finally(() => {
      if (playlistPlayPromise === guardedTask) {
        playlistPlayPromise = null;
        playlistPlayTargetId = undefined;
      }
      if (pendingPlaylistMediaId === mediaId) {
        pendingPlaylistMediaId = undefined;
      }
    });

    playlistPlayPromise = guardedTask;
    await playlistPlayPromise;
  }

  async function handleWindowMinimize() {
    if (!useWindowsCustomFrame) return;
    await tauriWindow.minimize();
  }

  async function handleWindowToggleMaximize() {
    if (!useWindowsCustomFrame) return;
    await tauriWindow.toggleMaximize();
    windowMaximized = await tauriWindow.isMaximized();
  }

  async function handleWindowClose() {
    await tauriWindow.close();
  }

  async function startAutoAsr(
    media: MediaItem,
    options: { statusMessage?: string; syncImportUi?: boolean; trackRetryProgress?: boolean } = {},
  ) {
    pendingSubtitleMediaId = media.id;
    isCancellingAsr = false;
    const {
      statusMessage = "素材已导入，正在离线生成字幕…",
      syncImportUi = true,
      trackRetryProgress = false,
    } = options;
    if (trackRetryProgress) {
      clearRetryAsrCompletionNotice();
      updateRetryAsrProgress(media.id, 6, "正在准备重新识别…");
    }
    try {
      const { jobId } = await backend.startAsrJob({ audioPath: media.audioPath });
      activeAsrJobId = jobId;
      if (trackRetryProgress) {
        startRetryAsrProgressDrift(media.id);
      }
      setStatus(statusMessage, "neutral");
    } catch (err) {
      console.error(err);
      pendingSubtitleMediaId = undefined;
      if (trackRetryProgress && retryAsrProgress?.mediaId === media.id) {
        stopRetryAsrProgressDrift();
        retryAsrProgress = undefined;
      }
      const message = formatError(err);
      if (syncImportUi) {
        importError = message;
        resetImportFlowState();
      }
      setStatus(message, "warning");
    }
  }

  async function retryAsrForMedia(mediaId: string) {
    if (activeAsrJobId) {
      setStatus("已有识别任务正在运行，请等待当前任务完成", "warning");
      return;
    }

    const media = libraryState.mediaItems.find((item) => item.id === mediaId);
    if (!media) {
      setStatus("未找到对应素材", "warning");
      return;
    }

    const ok = await confirmDialog.show(
      "重新识别字幕",
      [
        `将为《${media.title}》重新执行离线识别。`,
        media.subtitlePath
          ? "当前已绑定的字幕结果可能会被新的识别结果覆盖。"
          : "当前素材还没有字幕，本次会重新生成识别结果。",
        "如果识别过程中退出应用，本轮任务可能中断，结果也可能无法自动绑定回素材。",
        "确定继续吗？",
      ].join(""),
    );
    if (!ok) return;

    await startAutoAsr(media, {
      statusMessage: `正在重新识别《${media.title}》字幕…`,
      syncImportUi: false,
      trackRetryProgress: true,
    });
  }

  async function handleCancelAsr() {
    if (!activeAsrJobId || isCancellingAsr) return;
    isCancellingAsr = true;
    setStatus("正在取消当前识别任务…", "warning");
    try {
      await backend.cancelAsrJob();
    } catch (err) {
      console.error(err);
      isCancellingAsr = false;
      setStatus(formatError(err), "warning");
    }
  }

  /* ── Subtitle editor ───────────────────────────────────── */

  async function openSubtitleEditor(mediaId: string) {
    try {
      subtitleEditorNotice = undefined;
      subtitleEditorSaving = false;
      activeSubtitleDocument = await backend.getSubtitleDocument(mediaId);
      setActivePage("subtitle-editor");
    } catch (err) {
      console.error(err);
      setStatus(formatError(err), "warning");
    }
  }

  function handleCueChange(index: number, field: "text" | "secondaryText", value: string) {
    if (!activeSubtitleDocument) return;
    const cue = activeSubtitleDocument.cues[index];
    if (!cue) return;
    if (field === "text") {
      cue.text = value;
      cue.atoms = [];
    }
    else cue.secondaryText = value;
  }

  function handleSubtitleTitleChange(value: string) {
    if (!activeSubtitleDocument) return;
    activeSubtitleDocument.title = value;
  }

  async function saveSubtitleEditor() {
    if (!activeSubtitleDocument) { setStatus("没有可保存的字幕内容", "warning"); return; }
    if (subtitleEditorSaving) return;
    subtitleEditorSaving = true;
    subtitleEditorNotice = undefined;
    try {
      const saved = await backend.saveSubtitleDocument(
        activeSubtitleDocument.mediaId,
        activeSubtitleDocument.title.trim() || "未命名素材",
        activeSubtitleDocument.cues,
      );
      activeSubtitleDocument = saved;
      await refreshLibrary();
      if (currentMediaId === saved.mediaId) {
        await loadSubtitleFromPath(saved.subtitlePath);
        renderSubtitle(player.getSnapshot());
        await syncOverlay(player.getSnapshot());
      }
      subtitleEditorNotice = "字幕校对已保存，正在返回上一页…";
      setStatus("字幕校对已保存", "success");
      await waitForNextPaint();
      await new Promise((resolve) => setTimeout(resolve, 700));
      setActivePage(lastMainPage);
      subtitleEditorNotice = undefined;
    } catch (err) {
      console.error(err);
      subtitleEditorNotice = undefined;
      setStatus(formatError(err), "warning");
    } finally {
      subtitleEditorSaving = false;
    }
  }

  /* ── Import handlers ───────────────────────────────────── */

  function beginImportFlow(source: "local" | "online", initialMessage: string) {
    activeImportSource = source;
    importError = undefined;
    importSuccessKind = "bilingual";
    showImportSuccess = false;
    clearTimeout(importSuccessTimer);
    isCancellingAsr = false;
    resetScheduledProgressUpdate();
    importProgress = {
      active: true,
      stage: source === "online" ? "downloading" : "importing",
      message: initialMessage,
      percent: source === "online" ? 4 : 6,
    };
  }

  function resetImportFlowState() {
    stopTranslationProgressDrift();
    resetScheduledProgressUpdate();
    clearTimeout(importSuccessTimer);
    importProgress = { ...IMPORT_IDLE };
    activeImportSource = undefined;
    isCancellingAsr = false;
  }

  function closeImportSuccess() {
    showImportSuccess = false;
    importSuccessName = undefined;
    importSuccessKind = "bilingual";
    resetImportFlowState();
  }

  async function handleImportMedia() {
    const selected = await open({
      multiple: false,
      filters: [{
        name: "媒体",
        extensions: ["mp3", "wav", "m4a", "aac", "flac", "ogg", "opus", "mp4", "m4v", "mov", "webm", "mkv", "avi"],
      }],
    });
    if (!selected || Array.isArray(selected)) return;

    beginImportFlow("local", "正在导入媒体文件…");
    try {
      await waitForNextPaint();
      const media = await backend.importMedia(selected);
      importProgress = {
        active: true,
        stage: "importing",
        message: "媒体导入成功，准备生成字幕…",
        percent: getImportSettledPercent("local"),
      };
      importSuccessName = media.title;
      await refreshLibrary();
      // 不在导入时预加载音频到播放器：
      // 1. 避免与正在进行的 ASR 进程竞争 CPU/IO
      // 2. 避免触发 syncOverlay IPC 链
      // 用户点击播放列表中的具体条目时再按需加载
      // 注意：成功弹框会在 ASR 完成 + 翻译之后才显示
      await startAutoAsr(media);
    } catch (err) {
      console.error(err);
      importError = formatError(err);
      resetImportFlowState();
    }
  }

  async function handleImportOnlineMedia(url: string) {
    beginImportFlow("online", "正在准备在线视频下载…");
    try {
      await waitForNextPaint();
      const media = await backend.importOnlineMedia(url);
      importProgress = {
        active: true,
        stage: "importing",
        message: "在线视频已导入，准备生成字幕…",
        percent: getImportSettledPercent("online"),
      };
      importSuccessName = media.title;
      await refreshLibrary();
      await startAutoAsr(media);
    } catch (err) {
      console.error(err);
      importError = formatError(err);
      resetImportFlowState();
      throw err;
    } finally {
      // no-op: startup and settings no longer probe yt-dlp status
    }
  }

  async function handleImportSubtitle() {
    if (!currentMediaId) { setStatus("请先选择一个素材", "warning"); return; }
    const selected = await open({
      multiple: false,
      filters: [{ name: "字幕", extensions: ["srt", "vtt"] }],
    });
    if (!selected || Array.isArray(selected)) return;
    try {
      await loadSubtitleFromPath(selected);
      await backend.updateMediaSubtitle(currentMediaId, selected);
      await refreshLibrary();
      setStatus("手动字幕已导入并绑定", "success");
    } catch (err) {
      console.error(err);
      setStatus(formatError(err), "warning");
    }
  }

  /* ── Add to playlist ──────────────────────────────────── */

  async function handleAddToPlaylist(mediaId: string) {
    await backend.prependPlaybackItem(mediaId);
    await refreshLibrary();
    await playPlaylistItem(mediaId, true, false);
  }

  /* ── Overlay event handlers ────────────────────────────── */

  async function handleOverlayVisibleChange(visible: boolean) {
    settings = { ...settings, overlayVisible: visible };
    await persistSettings();
    if (visible) {
      await backend.showOverlay();
      await overlayBridge.updateStyle(settings.overlay);
    } else {
      await backend.hideOverlay();
    }
    await syncOverlay(player.getSnapshot());
    if (visible) {
      // Showing the always-on-top overlay can steal focus from the main window.
      await tauriWindow.setFocus();
    }
  }

  async function handleOverlayLockToggle() {
    overlayLocked = !overlayLocked;
    await emitTo("overlay", OVERLAY_LOCK_EVENT, { locked: overlayLocked });
  }

  async function handleOverlayStyleChange(overlay: OverlaySettings) {
    settings = { ...settings, overlay };
    await overlayBridge.updateStyle(overlay);
  }

  /* ── Model handlers ────────────────────────────────────── */

  async function handleDownloadModel(modelId: string, options?: { silent?: boolean, resuming?: boolean }) {
    try {
      const { jobId } = await backend.downloadModel(modelId);
      activeModelDownloadJobId = jobId;
      downloadingModelId = modelId;
      modelDownloadPercent = 0;
      const label = availableModels.find((m) => m.id === modelId)?.label ?? modelId;
      if (!options?.silent) {
        setStatus(options?.resuming ? `继续下载模型 ${label}` : `模型 ${label} 开始下载`, "neutral");
      }
    } catch (err) {
      console.error(err);
      if (!options?.silent) {
        setStatus("启动模型下载失败", "warning");
      }
      throw err;
    }
  }

  async function handleCancelModelDownload() {
    try {
      await backend.cancelModelDownload();
      isDownloadPaused = false;
      setStatus("模型下载已取消", "warning");
    } catch (err) {
      console.error(err);
    }
  }

  async function handlePauseModelDownload() {
    try {
      await backend.pauseModelDownload();
      isDownloadPaused = true;
      setStatus("模型下载已暂停", "neutral");
    } catch (err) {
      console.error(err);
    }
  }

  async function handleResumeModelDownload() {
    try {
      await backend.resumeModelDownload();
      isDownloadPaused = false;
      setStatus("模型下载已恢复", "neutral");
    } catch (err) {
      console.error(err);
    }
  }

  async function handleSelectModel(modelId: string, options?: { silent?: boolean }) {
    settings = { ...settings, selectedModel: modelId };
    await persistSettings();
    try {
      const s = await backend.getModelStatus(modelId);
      modelsStatusMap = new Map(modelsStatusMap).set(modelId, s);
    } catch { /* ignore */ }
    refreshModelLabels();
    const label = availableModels.find((m) => m.id === modelId)?.label ?? modelId;
    if (!options?.silent) {
      setStatus(`已切换为 ${label} 模型`, "success");
    }
  }

  async function refreshTranslationModelStatus() {
    try {
      translationModelStatus = await backend.getTranslationModelStatus();
      refreshTranslationModelLabels();
    } catch (err) {
      console.error(err);
      translationModelStatus = undefined;
      translationModelStatusLabel = "翻译模型状态读取失败";
      translationModelPathLabel = "";
    }
  }

  async function handleDownloadTranslationModel(options?: { silent?: boolean, resuming?: boolean }) {
    try {
      const { jobId } = await backend.downloadTranslationModel();
      activeTranslationModelDownloadJobId = jobId;
      translationModelDownloadPercent = 0;
      if (!options?.silent) {
        setStatus(options?.resuming ? "继续下载翻译模型" : "翻译模型开始下载", "neutral");
      }
    } catch (err) {
      console.error(err);
      if (!options?.silent) {
        setStatus("启动翻译模型下载失败", "warning");
      }
      throw err;
    }
  }

  async function maybeResumeTranslationModelDownload() {
    if (hasTriedAutoResumeTranslationModelDownload || activeTranslationModelDownloadJobId) return;
    hasTriedAutoResumeTranslationModelDownload = true;

    try {
      const resumable = await backend.getResumableTranslationModelDownload();
      if (!resumable) return;

      await handleDownloadTranslationModel({ silent: true, resuming: true });
      setStatus("检测到未完成下载，正在继续翻译模型", "neutral");
    } catch (err) {
      console.error(err);
    }
  }

  async function handleCancelTranslationModelDownload() {
    try {
      await backend.cancelTranslationModelDownload();
      isTranslationModelDownloadPaused = false;
      setStatus("翻译模型下载已取消", "warning");
    } catch (err) {
      console.error(err);
    }
  }

  async function handlePauseTranslationModelDownload() {
    try {
      await backend.pauseTranslationModelDownload();
      isTranslationModelDownloadPaused = true;
      setStatus("翻译模型下载已暂停", "neutral");
    } catch (err) {
      console.error(err);
    }
  }

  async function handleResumeTranslationModelDownload() {
    try {
      await backend.resumeTranslationModelDownload();
      isTranslationModelDownloadPaused = false;
      setStatus("翻译模型下载已恢复", "neutral");
    } catch (err) {
      console.error(err);
    }
  }

  async function handleDeleteTranslationModel() {
    if (!translationModelInfo) return;
    if (!await confirmDialog.show("删除翻译模型", `确定删除 ${translationModelInfo.label} 吗？删除后中文字幕翻译将不可用。`)) return;
    try {
      const result = await backend.deleteTranslationModel();
      await refreshTranslationModelStatus();
      setStatus(`翻译模型已删除，${fmtCleanup(result)}`, "success");
    } catch (err) {
      console.error(err);
      setStatus("删除翻译模型失败", "warning");
    }
  }

  async function beginOnboardingModelDownload(modelId: string) {
    onboardingSelectedModelId = modelId;
    onboardingError = undefined;
    onboardingDownloadPercent = 0;
    onboardingDownloadMessage = "正在准备模型下载…";
    onboardingStep = "downloading";

    await handleSelectModel(modelId, { silent: true });
    const status = await backend.getModelStatus(modelId);
    modelsStatusMap = new Map(modelsStatusMap).set(modelId, status);
    refreshModelLabels();

    if (status.installed) {
      onboardingDownloadPercent = 100;
      onboardingDownloadMessage = "模型已存在，已直接就绪。";
      onboardingStep = "ready";
      return;
    }

    await handleDownloadModel(modelId, { silent: true });
  }

  async function handleOnboardingModelSelect(modelId: string) {
    try {
      await beginOnboardingModelDownload(modelId);
    } catch (err) {
      onboardingError = formatError(err);
      onboardingDownloadMessage = "模型下载启动失败";
      onboardingStep = "downloading";
      setStatus(formatError(err), "warning");
    }
  }

  async function handleRetryOnboardingDownload() {
    if (!onboardingSelectedModelId) return;
    await handleOnboardingModelSelect(onboardingSelectedModelId);
  }

  function handleBackToOnboardingSelection() {
    onboardingError = undefined;
    onboardingDownloadPercent = 0;
    onboardingDownloadMessage = "正在准备模型下载…";
    onboardingStep = "select-model";
  }

  async function handleDeleteModel(modelId: string) {
    const label = availableModels.find((m) => m.id === modelId)?.label ?? modelId;
    if (!await confirmDialog.show("删除模型", `确定删除模型 ${label} 吗？删除后需要重新下载。`)) return;
    try {
      const r = await backend.deleteModel(modelId);
      const s = await backend.getModelStatus(modelId);
      modelsStatusMap = new Map(modelsStatusMap).set(modelId, s);
      refreshModelLabels();
      setStatus(`模型 ${label} 已删除，${fmtCleanup(r)}`, "success");
    } catch (err) {
      console.error(err);
      setStatus(`删除模型 ${label} 失败`, "warning");
    }
  }

  /* ── Danger zone ───────────────────────────────────────── */

  async function handleClearAllCache() {
    if (!await confirmDialog.show(
      "删除所有缓存",
      "全部已导入的素材、音频文件及字幕将被删除，资源列表与播放列表将被清空。离线模型与应用设置不受影响。",
    )) return;
    try {
      const r = await backend.clearMediaLibrary();
      // 重置播放器与前端状态
      await resetPlaybackUi();
      await refreshLibrary();
      setStatus(`缓存已清理，${fmtCleanup(r)}`, "success");
    } catch (err) { console.error(err); setStatus("清理缓存失败", "warning"); }
  }

  async function handleDeleteAllModels() {
    if (!await confirmDialog.show("删除所有离线模型", "所有已下载的识别模型和翻译模型都将被删除，需要重新下载后才能继续离线识别或生成中文字幕。")) return;
    try {
      const r = await backend.deleteDefaultModel();
      const allStatus = await backend.getAllModelsStatus();
      const newMap = new Map<string, ModelStatus>();
      for (const s of allStatus.models) newMap.set(s.modelId, s);
      modelsStatusMap = newMap;
      refreshModelLabels();
      await refreshTranslationModelStatus();
      setStatus(`所有模型已删除，${fmtCleanup(r)}`, "success");
    } catch (err) { console.error(err); setStatus("删除模型失败", "warning"); }
  }

  async function handleResetAppData() {
    if (!settingsReady) return;
    if (!await confirmDialog.show("删除全部数据", "所有离线模型、音频字幕缓存及应用设置都将被彻底清空，此操作不可逆！")) return;
    try {
      await clearPlaybackPersistenceState();
      const r = await backend.resetAppData();
      settings = await backend.getSettings();
      player.setPlaybackRate(settings.playbackRate);
      player.setVolume(settings.volume);
      showFirstRunOnboarding = true;
      onboardingStep = "select-model";
      onboardingSelectedModelId = undefined;
      onboardingDownloadPercent = 0;
      onboardingDownloadMessage = "正在准备模型下载…";
      onboardingError = undefined;
      const allStatus = await backend.getAllModelsStatus();
      const newMap = new Map<string, ModelStatus>();
      for (const s of allStatus.models) newMap.set(s.modelId, s);
      modelsStatusMap = newMap;
      refreshModelLabels();
      await refreshTranslationModelStatus();
      await backend.hideOverlay();
      await resetPlaybackUi();
      await refreshLibrary();
      setStatus(`应用数据已重置，${fmtCleanup(r)}`, "success");
    } catch (err) { console.error(err); setStatus("重置应用数据失败", "warning"); }
  }

  /* ── Keyboard shortcuts ────────────────────────────────── */

  function handleKeydown(e: KeyboardEvent) {
    if (!settingsReady) return;
    if (e.target instanceof HTMLInputElement || e.target instanceof HTMLSelectElement || e.target instanceof HTMLTextAreaElement) return;
    const shortcut = settings.shortcuts;

    if (e.code === shortcut.playPause) {
      e.preventDefault();
      if (hasMedia) void handleTogglePlayback("shortcut");
      return;
    }
    if (e.code === shortcut.previousTrack) {
      e.preventDefault();
      void playHistoryDirection(-1);
      return;
    }
    if (e.code === shortcut.nextTrack) {
      e.preventDefault();
      void playHistoryDirection(1);
      return;
    }
    if (e.code === shortcut.toggleOverlay) {
      e.preventDefault();
      void handleOverlayVisibleChange(!settings.overlayVisible);
      return;
    }
    if (e.code === shortcut.volumeUp) {
      e.preventDefault();
      applyVolume(settings.volume + 0.05);
      void commitVolume(false);
      return;
    }
    if (e.code === shortcut.volumeDown) {
      e.preventDefault();
      applyVolume(settings.volume - 0.05);
      void commitVolume(false);
      return;
    }
    if (e.code === shortcut.showTranslation) {
      e.preventDefault();
      void setSubtitleDisplayMode("translation");
      return;
    }
    if (e.code === shortcut.showOriginal) {
      e.preventDefault();
      void setSubtitleDisplayMode("original");
      return;
    }
    if (e.code === shortcut.showBilingual) {
      e.preventDefault();
      void setSubtitleDisplayMode("bilingual");
      return;
    }
  }

  /* ── onMount (init) ────────────────────────────────────── */

  onMount(async () => {
    let closingConfirmVisible = false;
    let closingApp = false;
    let unlistenWindowResized: (() => void) | undefined;
    const audioDebugEvents = [
      "loadstart",
      "loadedmetadata",
      "loadeddata",
      "canplay",
      "canplaythrough",
      "play",
      "playing",
      "pause",
      "waiting",
      "stalled",
      "suspend",
      "seeking",
      "seeked",
      "ended",
      "abort",
      "emptied",
      "error",
    ] as const;
    const onAudioDebugEvent = (event: Event) => {
      const type = event.type;
      const baseMessage = [
        `paused=${audioEl.paused}`,
        `readyState=${audioEl.readyState}`,
        `networkState=${audioEl.networkState}`,
        `currentTime=${audioEl.currentTime.toFixed(3)}`,
      ];

      if (type === "error") {
        baseMessage.push(`error=${describeMediaError(audioEl.error)}`);
        console.warn(baseMessage.join(" | "));
      }
    };
    // Init services
    subtitleEngine = new SubtitleEngine();
    player = new PlayerController(audioEl);

    for (const eventName of audioDebugEvents) {
      audioEl.addEventListener(eventName, onAudioDebugEvent);
    }

    useWindowsCustomFrame = navigator.userAgent.toLowerCase().includes("windows");
    if (useWindowsCustomFrame) {
      windowMaximized = await tauriWindow.isMaximized();
      unlistenWindowResized = await tauriWindow.onResized(async () => {
        windowMaximized = await tauriWindow.isMaximized();
      });
    }

    // Player subscription
    player.subscribe((s) => {
      snap = s;
      playbackAnchor = createPlaybackClockAnchor(s);
      hasMedia = player.hasMedia();
      renderSubtitle(s);
      void syncOverlay(s);
      if (!restoringPlaybackState) {
        queuePlaybackStatePersist(getPlaybackStateFromSnapshot(s));
      }
    });

    player.onEnded(() => {
      if (!settingsReady) return;
      if (settings.playlistMode === "single") {
        player.seek(0);
        void handleTogglePlayback("ended-single");
        return;
      }
      if (!currentMediaId || libraryState.playbackHistory.length < 2) return;
      const idx = libraryState.playbackHistory.findIndex((i) => i.mediaId === currentMediaId);
      if (idx === -1) return;
      const next = libraryState.playbackHistory[(idx + 1) % libraryState.playbackHistory.length];
      void loadMediaById(next.mediaId, true).then(() => handleTogglePlayback("ended-next"));
    });

    // Overlay lock event from overlay window
    const unlistenLock = await listen<{ locked: boolean }>(OVERLAY_LOCK_EVENT, ({ payload }) => {
      overlayLocked = payload.locked;
    });

    // Overlay close event from overlay window
    const unlistenClose = await listen(OVERLAY_CLOSE_EVENT, async () => {
      settings = { ...settings, overlayVisible: false };
      await persistSettings();
    });
    const unlistenAppClose = await appEvents.onCloseRequested(async (summary) => {
      if (closingApp || closingConfirmVisible) return;
      closingConfirmVisible = true;
      try {
        if (summary.hasActiveTasks) {
          await tauriWindow.show();
          await tauriWindow.unminimize();
          await tauriWindow.setFocus();
        }
        closingApp = true;
        const shouldExit = await handleAppCloseRequest(summary);
        if (!shouldExit) {
          closingApp = false;
        }
      } catch (err) {
        closingApp = false;
        setStatus(formatError(err), "warning");
      } finally {
        closingConfirmVisible = false;
      }
    });
    const unImportProgress = await importEvents.onProgress(({ stage, message, percent }) => {
      if (!importProgress.active) return;

      if (stage === "downloading") {
        if (activeImportSource !== "online") return;
        scheduleProgressUpdate({
          active: true,
          stage: "downloading",
          message,
          percent: Math.round(getImportStageOverallPercent(stage, percent, "online")),
        });
        return;
      }

      scheduleProgressUpdate({
        active: true,
        stage: "importing",
        message,
        percent: Math.round(getImportStageOverallPercent(stage, percent)),
      });
    });

    // ASR events
    const unAsrStarted = await asrEvents.onStarted(({ jobId }) => {
      activeAsrJobId = jobId;
      // 不再更新已移除的侧边栏状态，只记录 jobId
    });

    const unAsrProgress = await asrEvents.onProgress(({ jobId, stage, message, percent }) => {
      if (activeAsrJobId && activeAsrJobId !== jobId) return;
      console.debug("[ASR]", stage, message, percent);

      if (retryAsrProgress && pendingSubtitleMediaId === retryAsrProgress.mediaId) {
        if (stage === "preparing") {
          updateRetryAsrProgress(retryAsrProgress.mediaId, 12, "正在准备识别环境…");
        } else if (stage === "recognizing") {
          updateRetryAsrProgress(
            retryAsrProgress.mediaId,
            percent != null && percent > 0 ? 18 + (percent / 100) * 68 : 24,
            percent != null && percent > 0
              ? `正在重新识别… ${Math.round(percent)}%`
              : "正在重新识别…",
          );
        } else if (stage === "writing") {
          updateRetryAsrProgress(retryAsrProgress.mediaId, 92, "识别完成，正在写入字幕…");
        }
      }

      // 仅在导入流程进行中时更新进度条
      if (!importProgress.active) return;

      // 使用 requestAnimationFrame 批量更新，避免高频状态变更导致掉帧
      if (stage === "preparing") {
        scheduleProgressUpdate({
          active: true,
          stage: "preparing",
          message: "正在检查依赖和模型…",
          percent: getPreparingOverallPercent(),
        });
      } else if (stage === "recognizing") {
        scheduleProgressUpdate({
          active: true,
          stage: "recognizing",
          message: percent != null && percent > 0
            ? `正在离线识别字幕… ${Math.round(percent)}%`
            : "正在离线识别字幕…",
          percent: Math.round(getRecognizingOverallPercent(percent)),
        });
      } else if (stage === "writing") {
        scheduleProgressUpdate({
          active: true,
          stage: "recognizing",
          message: "识别完成，正在写入字幕…",
          percent: getWritingOverallPercent(),
        });
      }
    });

    const unAsrCompleted = await asrEvents.onCompleted(async ({ jobId, subtitlePath, detectedLanguage }) => {
      if (activeAsrJobId !== jobId) return;
      activeAsrJobId = undefined;
      isCancellingAsr = false;
      const mediaIdForSubtitle = pendingSubtitleMediaId;
      const isRetryFlow = Boolean(mediaIdForSubtitle && retryAsrProgress?.mediaId === mediaIdForSubtitle);
      try {
        let finalSubtitlePath = subtitlePath;
        let translationError: string | undefined;
        const shouldTranslateToChinese = !isChineseLanguage(detectedLanguage);
        if (mediaIdForSubtitle) {
          await backend.updateMediaSubtitle(mediaIdForSubtitle, subtitlePath);
          if (shouldTranslateToChinese) {
            if (isRetryFlow) {
              updateRetryAsrProgress(mediaIdForSubtitle, 96, "正在生成中文字幕…");
            }
            if (importProgress.active) {
              startTranslationProgressDrift();
              resetScheduledProgressUpdate();
              importProgress = {
                active: true,
                stage: "translating",
                message: "正在生成中文翻译…",
                percent: getTranslatingOverallPercent(),
              };
            }
            try {
              const latestTranslationModelStatus = await backend.getTranslationModelStatus();
              translationModelStatus = latestTranslationModelStatus;
              refreshTranslationModelLabels();
              if (!latestTranslationModelStatus.installed) {
                throw new Error("未安装翻译模型，请先到设置页下载 M2M100 418M");
              }
              await waitForNextPaint();
              const translated = await backend.translateMediaSubtitle(mediaIdForSubtitle, detectedLanguage);
              finalSubtitlePath = translated.subtitlePath;
            } catch (err) {
              console.error(err);
              translationError = formatError(err);
            }
          }
        }

        // 先更新状态文字（轻量操作），再延迟执行重量级 IPC + 渲染
        setStatus(
          isRetryFlow
            ? translationError
              ? `重新识别完成，但中文字幕生成失败：${translationError}`
              : "重新识别完成"
            : translationError
              ? `离线识别完成，但中文字幕生成失败：${translationError}`
              : shouldTranslateToChinese
                ? "离线识别完成，双语字幕已绑定"
                : "离线识别完成，原文字幕已绑定",
          translationError ? "warning" : "success",
        );

        importSuccessKind = translationError
          ? "translation-failed"
          : shouldTranslateToChinese
            ? "bilingual"
            : "original";

        // 整个流水线结束 → 显示成功弹框
        if (importProgress.active) {
          stopTranslationProgressDrift();
          resetScheduledProgressUpdate();
          importProgress = {
            active: true,
            stage: "done",
            message: importSuccessKind === "translation-failed"
              ? "导入完成，中文字幕生成失败"
              : importSuccessKind === "original"
                ? "导入完成，已生成原文字幕"
                : "导入完成，双语字幕已生成",
            percent: 100,
          };
        }

        // 延迟执行 refreshLibrary + loadSubtitle，让 UI 先完成 100% 进度渲染
        // 避免完成瞬间大量 IPC + DOM 更新挤占渲染帧
        await new Promise((r) => setTimeout(r, 50));

        await refreshLibrary();
        if (mediaIdForSubtitle && currentMediaId === mediaIdForSubtitle) {
          await loadSubtitleFromPath(finalSubtitlePath);
        }

        if (isRetryFlow && mediaIdForSubtitle) {
          finishRetryAsrNotice(
            mediaIdForSubtitle,
            translationError ? "重新识别完成，但中文字幕生成失败" : "重新识别完成",
          );
        }

        if (importProgress.active) {
          // 短暂显示 100% 后弹出成功提示
          clearTimeout(importSuccessTimer);
          importSuccessTimer = setTimeout(() => {
            showImportSuccess = true;
            activeImportSource = undefined;
          }, 550);
        }
      } catch (err) {
        console.error(err);
        if (isRetryFlow) {
          stopRetryAsrProgressDrift();
          retryAsrProgress = undefined;
        }
        setStatus("识别完成，但字幕绑定失败", "warning");
        resetImportFlowState();
      } finally {
        pendingSubtitleMediaId = undefined;
      }
    });

    const unAsrFailed = await asrEvents.onFailed(({ jobId, code, message }) => {
      if (activeAsrJobId && activeAsrJobId !== jobId) return;
      activeAsrJobId = undefined;
      isCancellingAsr = false;
      stopTranslationProgressDrift();
      stopRetryAsrProgressDrift();
      retryAsrProgress = undefined;
      pendingSubtitleMediaId = undefined;
      if (code === "asr_cancelled") {
        setStatus("已取消当前识别任务", "warning");
        resetImportFlowState();
        importError = undefined;
        return;
      }

      setStatus(`[${code}] ${message}`, "warning");
      if (importProgress.active) {
        resetImportFlowState();
        importError = `字幕生成失败: [${code}] ${message}`;
      }
    });

    // Model download events
    const unModelStarted = await modelEvents.onStarted(({ jobId, modelId }) => {
      activeModelDownloadJobId = jobId;
      downloadingModelId = modelId;
      modelDownloadPercent = 0;
      if (showFirstRunOnboarding && onboardingSelectedModelId === modelId) {
        onboardingStep = "downloading";
        onboardingError = undefined;
        onboardingDownloadPercent = 0;
        onboardingDownloadMessage = "正在连接模型下载源…";
      }
    });

    const unModelProgress = await modelEvents.onProgress(({ jobId, message, percent }) => {
      if (activeModelDownloadJobId && activeModelDownloadJobId !== jobId) return;
      console.debug("[Model Download]", message);
      if (percent != null) {
        modelDownloadPercent = Math.max(modelDownloadPercent, Math.round(percent));
      }
      if (showFirstRunOnboarding && onboardingStep === "downloading") {
        onboardingDownloadMessage = message;
        if (percent != null) {
          onboardingDownloadPercent = Math.max(onboardingDownloadPercent, Math.round(percent));
        }
      }
    });

    const unModelCompleted = await modelEvents.onCompleted(({ jobId, status }) => {
      if (activeModelDownloadJobId && activeModelDownloadJobId !== jobId) return;
      activeModelDownloadJobId = undefined;
      downloadingModelId = undefined;
      modelDownloadPercent = 0;
      isDownloadPaused = false;
      modelsStatusMap = new Map(modelsStatusMap).set(status.modelId, status);
      refreshModelLabels();
      if (showFirstRunOnboarding && onboardingSelectedModelId === status.modelId) {
        onboardingDownloadPercent = 100;
        onboardingDownloadMessage = "模型下载完成，可以开始使用了。";
        onboardingError = undefined;
        onboardingStep = "ready";
      }
      const label = availableModels.find((m) => m.id === status.modelId)?.label ?? status.modelId;
      if (!showFirstRunOnboarding) {
        if (modelDownloadSuccessTimer) clearTimeout(modelDownloadSuccessTimer);
        modelDownloadSuccessLabel = label;
        modelDownloadSuccessTimer = setTimeout(() => { modelDownloadSuccessLabel = undefined; }, 3000);
        setStatus(`模型 ${label} 下载完成`, "success");
      }
    });

    const unModelFailed = await modelEvents.onFailed(({ jobId, code, message }) => {
      if (activeModelDownloadJobId && activeModelDownloadJobId !== jobId) return;
      const failedModelId = downloadingModelId;
      activeModelDownloadJobId = undefined;
      downloadingModelId = undefined;
      modelDownloadPercent = 0;
      isDownloadPaused = false;
      if (showFirstRunOnboarding && onboardingStep === "downloading") {
        onboardingError = `[${code}] ${message}`;
        onboardingDownloadMessage = "模型下载失败，请重试。";
      }
      if (failedModelId) {
        void backend.getModelStatus(failedModelId).then((status) => {
          modelsStatusMap = new Map(modelsStatusMap).set(failedModelId, status);
          refreshModelLabels();
        }).catch((err) => {
          console.error(err);
        });
      }
      setStatus(`[${code}] ${message}`, "warning");
    });

    const unTranslationModelStarted = await translationModelEvents.onStarted(({ jobId }) => {
      activeTranslationModelDownloadJobId = jobId;
      translationModelDownloadPercent = 0;
      isTranslationModelDownloadPaused = false;
    });

    const unTranslationModelProgress = await translationModelEvents.onProgress(({ jobId, message, percent }) => {
      if (activeTranslationModelDownloadJobId && activeTranslationModelDownloadJobId !== jobId) return;
      console.debug("[Translation Model Download]", message);
      if (percent != null) {
        translationModelDownloadPercent = Math.max(
          translationModelDownloadPercent,
          Math.round(percent),
        );
      }
    });

    const unTranslationModelCompleted = await translationModelEvents.onCompleted(({ jobId, status }) => {
      if (activeTranslationModelDownloadJobId && activeTranslationModelDownloadJobId !== jobId) return;
      activeTranslationModelDownloadJobId = undefined;
      translationModelDownloadPercent = 0;
      isTranslationModelDownloadPaused = false;
      translationModelStatus = status;
      refreshTranslationModelLabels();
      setStatus("翻译模型下载完成", "success");
    });

    const unTranslationModelFailed = await translationModelEvents.onFailed(({ jobId, code, message }) => {
      if (activeTranslationModelDownloadJobId && activeTranslationModelDownloadJobId !== jobId) return;
      activeTranslationModelDownloadJobId = undefined;
      translationModelDownloadPercent = 0;
      isTranslationModelDownloadPaused = false;
      void refreshTranslationModelStatus();
      setStatus(`[${code}] ${message}`, "warning");
    });

    // Load settings
    try {
      settingsReady = false;
      settings = await backend.getSettings();
      const normalizedPlaybackState = getSavedPlaybackState();
      settings = { ...settings, playbackState: normalizedPlaybackState };
      lastSavedPlaybackState = normalizedPlaybackState;
      pendingPlaybackState = undefined;
      player.setPlaybackRate(settings.playbackRate);
      player.setVolume(settings.volume);
      showFirstRunOnboarding = FORCE_ONBOARDING || !settings.hasCompletedOnboarding;
      onboardingStep = settings.hasCompletedOnboarding ? "ready" : "select-model";
      if (FORCE_ONBOARDING) onboardingStep = "select-model";
      settingsReady = true;
      setStatus("设置已加载", "success");
      void initializeOverlayAfterStartup();
    } catch (err) {
      console.error(err);
      settings = {
        ...BOOTSTRAP_SETTINGS,
        overlay: { ...BOOTSTRAP_SETTINGS.overlay },
        shortcuts: { ...BOOTSTRAP_SETTINGS.shortcuts },
      };
      lastSavedPlaybackState = undefined;
      pendingPlaybackState = undefined;
      player.setPlaybackRate(settings.playbackRate);
      player.setVolume(settings.volume);
      showFirstRunOnboarding = FORCE_ONBOARDING || !settings.hasCompletedOnboarding;
      onboardingStep = settings.hasCompletedOnboarding ? "ready" : "select-model";
      if (FORCE_ONBOARDING) onboardingStep = "select-model";
      settingsReady = true;
      setStatus("读取设置失败，已使用默认配置", "warning");
      void initializeOverlayAfterStartup();
    }

    void hydrateInitialData();

    return () => {
      unlistenLock();
      unlistenAppClose();
      unlistenClose();
      unlistenWindowResized?.();
      unImportProgress();
      unAsrStarted();
      unAsrProgress();
      unAsrCompleted();
      unAsrFailed();
      unModelStarted();
      unModelProgress();
      unModelCompleted();
      unModelFailed();
      unTranslationModelStarted();
      unTranslationModelProgress();
      unTranslationModelCompleted();
      unTranslationModelFailed();
      clearTimeout(importSuccessTimer);
      clearTimeout(retryAsrNoticeTimer);
      clearInterval(translationProgressDriftTimer);
      clearInterval(retryAsrProgressDriftTimer);
      resetScheduledProgressUpdate();
      for (const eventName of audioDebugEvents) {
        audioEl.removeEventListener(eventName, onAudioDebugEvent);
      }
    };
  });
</script>

<svelte:window onkeydown={handleKeydown} />

<!-- Hidden audio element -->
<!-- svelte-ignore a11y_media_has_caption -->
<audio bind:this={audioEl} preload="metadata" style="display:none"></audio>

{#if modelDownloadSuccessLabel}
  <div class="model-download-toast">
    <span class="model-download-toast-icon">&#10003;</span>
    模型 {modelDownloadSuccessLabel} 下载成功
  </div>
{/if}

<main class="app-shell">
  <div class="window-drag-bar" class:window-drag-bar-with-controls={useWindowsCustomFrame}>
    <div class="window-drag-region" data-tauri-drag-region>
      {#if useWindowsCustomFrame}
        <span class="window-caption">muyu</span>
      {/if}
    </div>
    {#if useWindowsCustomFrame}
      <div class="window-controls">
        <button
          class="window-control-btn"
          type="button"
          title="最小化"
          aria-label="最小化"
          onclick={() => { void handleWindowMinimize(); }}
        >
          <svg width="10" height="10" viewBox="0 0 10 10" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round">
            <line x1="1.5" y1="5" x2="8.5" y2="5" />
          </svg>
        </button>
        <button
          class="window-control-btn"
          type="button"
          title={windowMaximized ? "还原" : "最大化"}
          aria-label={windowMaximized ? "还原" : "最大化"}
          onclick={() => { void handleWindowToggleMaximize(); }}
        >
          {#if windowMaximized}
            <svg width="10" height="10" viewBox="0 0 10 10" fill="none" stroke="currentColor" stroke-width="1.2">
              <path d="M2.5 1.5h5v5h-5z" />
              <path d="M1.5 3.5v5h5" />
            </svg>
          {:else}
            <svg width="10" height="10" viewBox="0 0 10 10" fill="none" stroke="currentColor" stroke-width="1.2">
              <rect x="1.75" y="1.75" width="6.5" height="6.5" />
            </svg>
          {/if}
        </button>
        <button
          class="window-control-btn window-control-btn-close"
          type="button"
          title="关闭"
          aria-label="关闭"
          onclick={() => { void handleWindowClose(); }}
        >
          <svg width="10" height="10" viewBox="0 0 10 10" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round">
            <line x1="2" y1="2" x2="8" y2="8" />
            <line x1="8" y1="2" x2="2" y2="8" />
          </svg>
        </button>
      </div>
    {/if}
  </div>

  {#if settingsReady}
    <Sidebar
      {activePage}
      onNavigate={setActivePage}
    />

    <section
      class="content"
      class:content-player={activePage === "playlist"}
      class:content-resources={activePage === "resources"}
    >
      {#if activePage === "import"}
        <div class="page-transition" in:fade={{ duration: 160, delay: 40 }}>
        <ImportPage
          progress={importProgress}
          {importError}
          {importSuccessName}
          {importSuccessKind}
          showSuccess={showImportSuccess}
          canCancel={Boolean(activeAsrJobId) && importProgress.stage !== "done"}
          {isCancellingAsr}
          onImportMedia={handleImportMedia}
          onImportOnline={handleImportOnlineMedia}
          onCancel={() => { void handleCancelAsr(); }}
          onDismissError={() => { importError = undefined; }}
          onImportSuccessClose={closeImportSuccess}
          onGoToResources={() => {
            closeImportSuccess();
            setActivePage("resources");
          }}
        />
        </div>
      {:else if activePage === "resources"}
        <div class="page-transition" in:fade={{ duration: 160, delay: 40 }}>
        <ResourceListPage
          items={libraryState.mediaItems}
          retryingMediaId={retryAsrProgress?.mediaId}
          retryingProgress={retryAsrProgress?.percent ?? 0}
          retryingMessage={retryAsrProgress?.message}
          retryCompletedMediaId={retryAsrCompletedMediaId}
          retryCompletedMessage={retryAsrCompletedMessage}
          asrBusy={Boolean(activeAsrJobId)}
          onRetryAsr={(id) => void retryAsrForMedia(id)}
          onEditSubtitle={(id) => void openSubtitleEditor(id)}
          onDeleteMedia={(id) => void deleteMediaById(id)}
          onAddToPlaylist={(id) => void handleAddToPlaylist(id)}
        />
        </div>
      {:else if activePage === "playlist"}
        <div class="page-transition" in:fade={{ duration: 160, delay: 40 }}>
        <PlayerPage
          {snap}
          {subtitleCues}
          {playbackAnchor}
          subtitleDisplayMode={settings.subtitleDisplayMode}
          playlist={libraryState.playbackHistory}
          {pendingPlaylistMediaId}
          {currentMediaId}
          onToggleCurrentItem={() => void handleTogglePlayback("playlist-current-item")}
          onSeek={(ms) => player.seek(ms)}
          onSubtitleDisplayModeChange={(mode) => { void setSubtitleDisplayMode(mode); }}
          onPlayItem={(id) => { void playPlaylistItem(id, true); }}
          onRemoveItem={(id) => { void removePlaybackItem(id); }}
        />
        </div>
      {:else if activePage === "settings"}
        <div class="page-transition" in:fade={{ duration: 160, delay: 40 }}>
        <SettingsPage
          {settings}
          {availableModels}
          {modelsStatusMap}
          {translationModelInfo}
          {translationModelStatus}
          isDownloading={Boolean(activeModelDownloadJobId)}
          {downloadingModelId}
          {modelDownloadPercent}
          {isDownloadPaused}
          isTranslationDownloading={Boolean(activeTranslationModelDownloadJobId)}
          translationDownloadPercent={translationModelDownloadPercent}
          isTranslationDownloadPaused={isTranslationModelDownloadPaused}
          {overlayLocked}
          onOverlayVisibleChange={handleOverlayVisibleChange}
          onOverlayLockToggle={handleOverlayLockToggle}
          onOverlayStyleChange={handleOverlayStyleChange}
          onOverlayStyleCommit={persistSettings}
          onDownloadModel={handleDownloadModel}
          onCancelDownload={handleCancelModelDownload}
          onPauseDownload={handlePauseModelDownload}
          onResumeDownload={handleResumeModelDownload}
          onSelectModel={handleSelectModel}
          onDeleteModel={handleDeleteModel}
          onDownloadTranslationModel={handleDownloadTranslationModel}
          onCancelTranslationDownload={handleCancelTranslationModelDownload}
          onPauseTranslationDownload={handlePauseTranslationModelDownload}
          onResumeTranslationDownload={handleResumeTranslationModelDownload}
          onDeleteTranslationModel={handleDeleteTranslationModel}
          onShortcutChange={(shortcuts) => {
            settings = { ...settings, shortcuts };
          }}
          onShortcutCommit={persistSettings}
          onOpenOnboarding={openOnboardingPreview}
          showOnboardingPreviewEntry={SHOW_ONBOARDING_PREVIEW_ENTRY}
          onClearAllCache={handleClearAllCache}
          onDeleteAllModels={handleDeleteAllModels}
          onResetAppData={handleResetAppData}
        />
        </div>
      {:else if activePage === "about"}
        <div class="page-transition" in:fade={{ duration: 160, delay: 40 }}>
        <AboutPage avatarSrc={aboutAvatarSrc} />
        </div>
      {:else if activePage === "subtitle-editor"}
        <div class="page-transition" in:fade={{ duration: 160, delay: 40 }}>
        <SubtitleEditor
          document={activeSubtitleDocument}
          lastMainPage={lastMainPage}
          saveNotice={subtitleEditorNotice}
          isSaving={subtitleEditorSaving}
          onBack={() => {
            subtitleEditorNotice = undefined;
            subtitleEditorSaving = false;
            setActivePage(lastMainPage);
          }}
          onSave={() => void saveSubtitleEditor()}
          onTitleChange={handleSubtitleTitleChange}
          onCueChange={handleCueChange}
        />
        </div>
      {/if}
    </section>

    <PlayerBar
      {snap}
      {hasMedia}
      {audioFileLabel}
      {subtitleFileLabel}
      {cueTiming}
      overlayVisible={settings.overlayVisible}
      playbackRate={settings.playbackRate}
      playlistMode={settings.playlistMode}
      volume={settings.volume}
      onTogglePlayback={() => void handleTogglePlayback("player-bar")}
      onSeek={(ms) => player.seek(ms)}
      onRateChange={async (rate) => {
        settings = { ...settings, playbackRate: rate };
        player.setPlaybackRate(rate);
        await persistSettings();
        setStatus(`播放倍率已更新为 ${rate.toFixed(2)}x`, "success");
      }}
      onPlaylistModeChange={async (mode: PlaylistMode) => {
        settings = { ...settings, playlistMode: mode };
        await persistSettings();
        setStatus(mode === "single" ? "已切换为单曲循环" : "已切换为顺序播放", "success");
      }}
      onToggleOverlayVisible={() => { void handleOverlayVisibleChange(!settings.overlayVisible); }}
      onToggleMute={() => { void toggleMute(); }}
      onVolumeChange={(volume) => applyVolume(volume)}
      onVolumeCommit={() => { void commitVolume(); }}
      onPrevTrack={() => { void playHistoryDirection(-1); }}
      onNextTrack={() => { void playHistoryDirection(1); }}
    />
  {:else}
    <section class="content"></section>
  {/if}
</main>

<ConfirmDialog bind:this={confirmDialog} />

{#if settingsReady && showFirstRunOnboarding}
  <FirstRunOnboarding
    topInset={useWindowsCustomFrame ? 48 : 0}
    step={onboardingStep}
    models={availableModels}
    {modelsStatusMap}
    selectedModelId={onboardingSelectedModelId}
    downloadPercent={onboardingDownloadPercent}
    downloadMessage={onboardingDownloadMessage}
    error={onboardingError}
    modelGuides={ONBOARDING_MODEL_GUIDES}
    onSelectModel={(id) => { void handleOnboardingModelSelect(id); }}
    onRetry={() => { void handleRetryOnboardingDownload(); }}
    onBack={handleBackToOnboardingSelection}
    onSkip={() => { void handleOnboardingSkip(); }}
    onStart={() => { void handleOnboardingStart(); }}
  />
{/if}
