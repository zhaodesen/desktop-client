<script lang="ts">
  import { onMount, tick } from "svelte";
  import { fade } from "svelte/transition";
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { emitTo, listen } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { open } from "@tauri-apps/plugin-dialog";
  import { OVERLAY_CLOSE_EVENT, OVERLAY_LOCK_EVENT } from "./shared/events";
  import { appEvents, asrEvents, backend, importEvents, modelEvents, overlayBridge } from "./shared/tauri";
  import type {
    AppSettings,
    CleanupResult,
    ImportProgress,
    LibraryState,
    MediaItem,
    ModelInfo,
    ModelStatus,
    OverlaySettings,
    PlaybackSnapshot,
    PlaylistMode,
    ShutdownTaskSummary,
    SubtitleCue,
    SubtitleDocument,
    ThemeMode,
  } from "./shared/types";
  import { PlayerController } from "./main/player-controller";
  import { parseSubtitleText } from "./main/subtitle-parser";
  import { SubtitleEngine } from "./main/subtitle-engine";
  import { formatDuration } from "./shared/utils";

  import Sidebar from "./lib/Sidebar.svelte";
  import ImportPage from "./lib/ImportPage.svelte";
  import ResourceListPage from "./lib/ResourceListPage.svelte";
  import PlayerPage from "./lib/PlayerPage.svelte";
  import SettingsPage from "./lib/SettingsPage.svelte";
  import SubtitleEditor from "./lib/SubtitleEditor.svelte";
  import ConfirmDialog from "./lib/ConfirmDialog.svelte";
  import FirstRunOnboarding from "./lib/FirstRunOnboarding.svelte";
  import MainTourOverlay from "./lib/MainTourOverlay.svelte";

  import "./styles.css";

  /* ── Constants ─────────────────────────────────────────── */

  const DEFAULT_SETTINGS: AppSettings = {
    playbackRate: 1,
    volume: 1,
    overlayVisible: false,
    overlay: {
      fontSize: 34,
      opacity: 1.0,
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
    themeMode: "dark" as ThemeMode,
  };

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

  const MAIN_TOUR_STEPS = [
    {
      id: "import",
      page: "import" as const,
      title: "从导入开始",
      description: "这里负责把本地音视频或在线视频拉进应用。导入后会自动开始离线识别并生成双语字幕。",
      hint: "先点击“导入”，再选择文件导入或在线视频导入。",
    },
    {
      id: "resources",
      page: "resources" as const,
      title: "资源列表",
      description: "所有已导入的素材都会在这里集中管理，你可以查看状态、编辑字幕，或把条目加入播放列表。",
      hint: "导入完成后，优先来这里检查字幕是否已经生成。",
    },
    {
      id: "playlist",
      page: "playlist" as const,
      title: "播放列表",
      description: "这里用来播放最近使用或手动加入的素材，可以边听边看字幕，也能直接重跑识别任务。",
      hint: "适合复听、复查和快速切换多个素材。",
    },
    {
      id: "settings",
      page: "settings" as const,
      title: "设置",
      description: "这里可以更换离线模型、调整悬浮字幕样式、修改快捷键，以及清理缓存数据。",
      hint: "如果以后想切模型或重下模型，就来这里。",
    },
  ];

  /* ── State ─────────────────────────────────────────────── */

  let settings = $state<AppSettings>({ ...DEFAULT_SETTINGS, overlay: { ...DEFAULT_SETTINGS.overlay } });
  let libraryState = $state<LibraryState>({ mediaItems: [], playbackHistory: [] });
  let activePage = $state("import");
  let lastMainPage = $state<"import" | "resources" | "playlist" | "settings">("import");

  let currentMediaId = $state<string | undefined>(undefined);
  let pendingPlaylistMediaId = $state<string | undefined>(undefined);
  let activeAsrJobId = $state<string | undefined>(undefined);
  let activeModelDownloadJobId = $state<string | undefined>(undefined);
  let pendingSubtitleMediaId = $state<string | undefined>(undefined);
  let overlayLocked = $state(false);

  // Import UI state: progress tracks the entire pipeline
  const IMPORT_IDLE: ImportProgress = { active: false, stage: "done", message: "", percent: 0 };
  let importProgress = $state<ImportProgress>({ ...IMPORT_IDLE });
  let importError = $state<string | undefined>(undefined);
  let importSuccessName = $state<string | undefined>(undefined);
  let showImportSuccess = $state(false);
  let importSuccessTimer: ReturnType<typeof setTimeout> | undefined;
  let isCancellingAsr = $state(false);

  let availableModels = $state<ModelInfo[]>([]);
  let modelsStatusMap = $state<Map<string, ModelStatus>>(new Map());
  let activeSubtitleDocument = $state<SubtitleDocument | undefined>(undefined);
  let activeImportSource = $state<"local" | "online" | undefined>(undefined);
  let showFirstRunOnboarding = $state(false);
  let onboardingStep = $state<"select-model" | "downloading" | "ready">("select-model");
  let onboardingSelectedModelId = $state<string | undefined>(undefined);
  let onboardingDownloadPercent = $state(0);
  let onboardingDownloadMessage = $state("正在准备模型下载…");
  let onboardingError = $state<string | undefined>(undefined);
  let showMainTour = $state(false);
  let mainTourStepIndex = $state(0);
  let mainTourTargetRect = $state<
    | { top: number; left: number; right: number; bottom: number; width: number; height: number }
    | undefined
  >(undefined);

  // Player state (published by PlayerController)
  let snap = $state<PlaybackSnapshot>({ playing: false, currentTimeMs: 0, durationMs: 0, rate: 1, volume: 1 });
  let hasMedia = $state(false);
  let audioFileLabel = $state("未选择素材");
  let subtitleFileLabel = $state("未生成字幕");
  let cueTiming = $state("--:-- ~ --:--");
  let currentText = $state("等待播放");
  let currentSecondaryText = $state("");
  let subtitleCues = $state<SubtitleCue[]>([]);
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

  // syncOverlay 节流：记录上次 IPC 时间，避免高频调用
  let lastOverlaySyncAt = 0;

  // ASR 进度更新 requestAnimationFrame 节流：
  // 即使 Rust 侧已节流到 1 秒 1 次，Svelte 的同步 DOM diff 仍可能导致掉帧。
  // 用 rAF 将状态更新推迟到下一个渲染帧，避免在高频事件回调中直接触发重排。
  let pendingProgressUpdate: ImportProgress | null = null;
  let progressRafId = 0;
  let mediaLoadRequestId = 0;
  let playlistPlayPromise: Promise<void> | null = null;
  let playlistPlayTargetId: string | undefined;

  function scheduleProgressUpdate(next: ImportProgress) {
    pendingProgressUpdate = next;
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

  /* ── Theme ─────────────────────────────────────────────── */

  function applyTheme(mode: ThemeMode) {
    let resolved: "dark" | "light" = mode === "system"
      ? (window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light")
      : mode;
    document.documentElement.setAttribute("data-theme", resolved);
  }

  $effect(() => {
    applyTheme(settings.themeMode);
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

  function fmtCleanup(r: CleanupResult): string {
    return `已删除 ${r.deletedFiles} 个文件，${r.deletedDirs} 个目录`;
  }

  function waitForNextPaint() {
    return new Promise<void>((resolve) => {
      requestAnimationFrame(() => resolve());
    });
  }

  function getCurrentMedia(): MediaItem | undefined {
    return libraryState.mediaItems.find((i) => i.id === currentMediaId);
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

    const result = await backend.shutdownAndExit();
    if (result.cancelledTasks.length > 0) {
      setStatus(`正在关闭后台任务并退出应用：${result.cancelledTasks.join("、")}`, "warning");
    } else {
      setStatus("正在退出应用…", "warning");
    }
    return true;
  }

  function setActivePage(page: string) {
    if (page === "import" || page === "resources" || page === "playlist" || page === "settings") {
      lastMainPage = page;
    }
    activePage = page;
  }

  function updateMainTourTargetRect() {
    if (!showMainTour) {
      mainTourTargetRect = undefined;
      return;
    }
    const step = MAIN_TOUR_STEPS[mainTourStepIndex];
    const el = document.querySelector<HTMLElement>(`[data-guide-id="${step.id}"]`);
    if (!el) {
      mainTourTargetRect = undefined;
      return;
    }
    const rect = el.getBoundingClientRect();
    mainTourTargetRect = {
      top: rect.top,
      left: rect.left,
      right: rect.right,
      bottom: rect.bottom,
      width: rect.width,
      height: rect.height,
    };
  }

  async function openMainTour() {
    mainTourStepIndex = 0;
    showMainTour = true;
    setActivePage("import");
    await tick();
    updateMainTourTargetRect();
  }

  async function completeMainTour() {
    showMainTour = false;
    mainTourTargetRect = undefined;
    setActivePage("import");
    if (settings.hasSeenMainTour) return;
    settings = { ...settings, hasSeenMainTour: true };
    await persistSettings();
  }

  async function handleMainTourNext() {
    const nextIndex = mainTourStepIndex + 1;
    if (nextIndex >= MAIN_TOUR_STEPS.length) {
      await completeMainTour();
      return;
    }

    mainTourStepIndex = nextIndex;
    setActivePage(MAIN_TOUR_STEPS[nextIndex].page);
    await tick();
    updateMainTourTargetRect();
  }

  async function handleOnboardingStart() {
    showFirstRunOnboarding = false;
    onboardingError = undefined;
    onboardingStep = "ready";
    settings = { ...settings, hasCompletedOnboarding: true };
    await persistSettings();
    setActivePage("import");

    if (!settings.hasSeenMainTour) {
      await openMainTour();
    }
  }

  /* ── Persist settings ──────────────────────────────────── */

  async function persistSettings() {
    settings = await backend.updateSettings(settings);
    await overlayBridge.updateStyle(settings.overlay);
  }

  /* ── Model UI helpers ──────────────────────────────────── */

  function refreshModelLabels() {
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

  /* ── Subtitle helpers ──────────────────────────────────── */

  function isChineseLanguage(code?: string): boolean {
    return code?.trim().toLowerCase().startsWith("zh") ?? false;
  }

  function getDisplayedCue(cue?: SubtitleCue): SubtitleCue | undefined {
    if (!cue) return undefined;
    if (settings.subtitleDisplayMode === "original") {
      return { ...cue, secondaryText: undefined };
    }
    if (settings.subtitleDisplayMode === "translation") {
      return {
        ...cue,
        text: cue.secondaryText?.trim() ?? "",
        secondaryText: undefined,
      };
    }
    return cue;
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
    // 守卫1：悬浮窗隐藏时跳过，避免无效 IPC
    if (!settings.overlayVisible) return;
    // 守卫2：100ms 节流，播放时最多 10fps IPC，避免 ticker+timeupdate 双重触发叠加
    const now = Date.now();
    if (now - lastOverlaySyncAt < 100) return;
    lastOverlaySyncAt = now;

    const media = getCurrentMedia();
    const ctx = subtitleEngine.getContext(s.currentTimeMs);
    await overlayBridge.render({
      fileLabel: media?.title,
      previous: undefined,
      current: getDisplayedCue(ctx.current),
      next: undefined,
      playback: s,
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
  }

  function createMediaLoadRequestId() {
    mediaLoadRequestId += 1;
    return mediaLoadRequestId;
  }

  function isLatestMediaLoadRequest(requestId: number) {
    return requestId === mediaLoadRequestId;
  }

  async function loadMediaById(mediaId: string, record: boolean, requestId = createMediaLoadRequestId()) {
    const media = libraryState.mediaItems.find((i) => i.id === mediaId);
    if (!media) {
      if (isLatestMediaLoadRequest(requestId)) {
        setStatus("未找到对应素材", "warning");
      }
      return false;
    }

    await player.loadUrl(convertFileSrc(media.audioPath));
    if (!isLatestMediaLoadRequest(requestId)) return false;

    player.setPlaybackRate(settings.playbackRate);
    player.setVolume(settings.volume);
    currentMediaId = media.id;
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
    const nextVolume = Math.max(0, Math.min(1, volume));
    settings = { ...settings, volume: nextVolume };
    player.setVolume(nextVolume);
  }

  async function commitVolume(showFeedback = true) {
    await persistSettings();
    if (showFeedback) {
      setStatus(`音量已调整为 ${Math.round(settings.volume * 100)}%`, "success");
    }
  }

  async function setSubtitleDisplayMode(mode: AppSettings["subtitleDisplayMode"], showFeedback = true) {
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

  async function playPlaylistItem(mediaId: string, autoplay = false) {
    if (playlistPlayPromise && playlistPlayTargetId === mediaId) {
      await playlistPlayPromise;
      return;
    }

    const requestId = createMediaLoadRequestId();
    pendingPlaylistMediaId = mediaId;
    playlistPlayTargetId = mediaId;

    const task = (async () => {
      const loaded = await loadMediaById(mediaId, true, requestId);
      if (autoplay && loaded && isLatestMediaLoadRequest(requestId)) {
        await player.play();
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
    options: { statusMessage?: string; syncImportUi?: boolean } = {},
  ) {
    pendingSubtitleMediaId = media.id;
    isCancellingAsr = false;
    const {
      statusMessage = "素材已导入，正在离线生成字幕…",
      syncImportUi = true,
    } = options;
    try {
      const { jobId } = await backend.startAsrJob({ audioPath: media.audioPath });
      activeAsrJobId = jobId;
      setStatus(statusMessage, "neutral");
    } catch (err) {
      console.error(err);
      pendingSubtitleMediaId = undefined;
      const message = formatError(err);
      if (syncImportUi) {
        importError = message;
        resetImportFlowState();
      }
      setStatus(message, "warning");
    }
  }

  async function retryAsrForMedia(mediaId: string) {
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
    if (field === "text") cue.text = value;
    else cue.secondaryText = value;
  }

  async function saveSubtitleEditor() {
    if (!activeSubtitleDocument) { setStatus("没有可保存的字幕内容", "warning"); return; }
    const saved = await backend.saveSubtitleDocument(
      activeSubtitleDocument.mediaId,
      activeSubtitleDocument.cues,
    );
    activeSubtitleDocument = saved;
    await refreshLibrary();
    if (currentMediaId === saved.mediaId) {
      await loadSubtitleFromPath(saved.subtitlePath);
      renderSubtitle(player.getSnapshot());
      await syncOverlay(player.getSnapshot());
    }
    setStatus("字幕校对已保存", "success");
  }

  /* ── Import handlers ───────────────────────────────────── */

  function beginImportFlow(source: "local" | "online", initialMessage: string) {
    activeImportSource = source;
    importError = undefined;
    showImportSuccess = false;
    clearTimeout(importSuccessTimer);
    isCancellingAsr = false;
    resetScheduledProgressUpdate();
    importProgress = {
      active: true,
      stage: source === "online" ? "downloading" : "importing",
      message: initialMessage,
      percent: source === "online" ? 2 : 5,
    };
  }

  function resetImportFlowState() {
    resetScheduledProgressUpdate();
    clearTimeout(importSuccessTimer);
    importProgress = { ...IMPORT_IDLE };
    activeImportSource = undefined;
    isCancellingAsr = false;
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
      importProgress = { active: true, stage: "importing", message: "媒体导入成功，准备生成字幕…", percent: 15 };
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
      importProgress = { active: true, stage: "importing", message: "在线视频已下载，准备生成字幕…", percent: 15 };
      importSuccessName = media.title;
      await refreshLibrary();
      await startAutoAsr(media);
    } catch (err) {
      console.error(err);
      importError = formatError(err);
      resetImportFlowState();
      throw err;
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
    await backend.recordPlayback(mediaId);
    await refreshLibrary();
    setStatus("已加入播放列表", "success");
  }

  /* ── Overlay event handlers ────────────────────────────── */

  async function handleOverlayVisibleChange(visible: boolean) {
    settings = { ...settings, overlayVisible: visible };
    await persistSettings();
    if (visible) await backend.showOverlay();
    else await backend.hideOverlay();
    await syncOverlay(player.getSnapshot());
  }

  async function handleOverlayLockToggle() {
    overlayLocked = !overlayLocked;
    await emitTo("overlay", OVERLAY_LOCK_EVENT, { locked: overlayLocked });
  }

  async function handleOverlayStyleChange(overlay: OverlaySettings) {
    settings = { ...settings, overlay };
    await overlayBridge.updateStyle(overlay);
  }

  async function handleThemeChange(mode: ThemeMode) {
    settings = { ...settings, themeMode: mode };
    await persistSettings();
  }

  /* ── Model handlers ────────────────────────────────────── */

  async function handleDownloadModel(modelId: string, options?: { silent?: boolean }) {
    try {
      const { jobId } = await backend.downloadModel(modelId);
      activeModelDownloadJobId = jobId;
      const label = availableModels.find((m) => m.id === modelId)?.label ?? modelId;
      if (!options?.silent) {
        setStatus(`模型 ${label} 开始下载`, "neutral");
      }
    } catch (err) {
      console.error(err);
      if (!options?.silent) {
        setStatus("启动模型下载失败", "warning");
      }
      throw err;
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
    if (!await confirmDialog.show("删除所有离线模型", "所有已下载的模型将被删除，需要重新下载才能离线识别。")) return;
    try {
      const r = await backend.deleteDefaultModel();
      const allStatus = await backend.getAllModelsStatus();
      const newMap = new Map<string, ModelStatus>();
      for (const s of allStatus.models) newMap.set(s.modelId, s);
      modelsStatusMap = newMap;
      refreshModelLabels();
      setStatus(`所有模型已删除，${fmtCleanup(r)}`, "success");
    } catch (err) { console.error(err); setStatus("删除模型失败", "warning"); }
  }

  async function handleResetAppData() {
    if (!await confirmDialog.show("删除全部数据", "所有离线模型、音频字幕缓存及应用设置都将被彻底清空，此操作不可逆！")) return;
    try {
      const r = await backend.resetAppData();
      settings = { ...DEFAULT_SETTINGS, overlay: { ...DEFAULT_SETTINGS.overlay } };
      player.setPlaybackRate(settings.playbackRate);
      player.setVolume(settings.volume);
      showFirstRunOnboarding = true;
      showMainTour = false;
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
      await backend.hideOverlay();
      await resetPlaybackUi();
      await refreshLibrary();
      setStatus(`应用数据已重置，${fmtCleanup(r)}`, "success");
    } catch (err) { console.error(err); setStatus("重置应用数据失败", "warning"); }
  }

  /* ── Keyboard shortcuts ────────────────────────────────── */

  function handleKeydown(e: KeyboardEvent) {
    if (e.target instanceof HTMLInputElement || e.target instanceof HTMLSelectElement || e.target instanceof HTMLTextAreaElement) return;
    const shortcut = settings.shortcuts;

    if (e.code === shortcut.playPause) {
      e.preventDefault();
      if (hasMedia) void player.togglePlayback();
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
    // Init services
    subtitleEngine = new SubtitleEngine();
    player = new PlayerController(audioEl);

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
      hasMedia = player.hasMedia();
      renderSubtitle(s);
      void syncOverlay(s);
    });

    player.onEnded(() => {
      if (settings.playlistMode === "single") {
        player.seek(0);
        void player.togglePlayback();
        return;
      }
      if (!currentMediaId || libraryState.playbackHistory.length < 2) return;
      const idx = libraryState.playbackHistory.findIndex((i) => i.mediaId === currentMediaId);
      if (idx === -1) return;
      const next = libraryState.playbackHistory[(idx + 1) % libraryState.playbackHistory.length];
      void loadMediaById(next.mediaId, true).then(() => player.togglePlayback());
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
    const unlistenWindowClose = await tauriWindow.onCloseRequested(async (event) => {
      event.preventDefault();
      if (closingApp || closingConfirmVisible) return;
      closingConfirmVisible = true;
      try {
        closingApp = true;
        const shouldExit = await handleAppCloseRequest();
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

    const handleViewportUpdate = () => {
      updateMainTourTargetRect();
    };
    window.addEventListener("resize", handleViewportUpdate);
    document.addEventListener("scroll", handleViewportUpdate, true);

    const unImportProgress = await importEvents.onProgress(({ stage, message, percent }) => {
      if (!importProgress.active) return;

      if (stage === "downloading") {
        if (activeImportSource !== "online") return;
        const overall = percent != null ? 2 + (percent / 100) * 10 : 6;
        scheduleProgressUpdate({
          active: true,
          stage: "downloading",
          message,
          percent: Math.round(overall),
        });
        return;
      }

      const localFallbackPercent: Record<"copying" | "extracting" | "registering", number> = {
        copying: 8,
        extracting: 10,
        registering: 15,
      };
      const onlineFallbackPercent: Record<"copying" | "extracting" | "registering", number> = {
        copying: 13,
        extracting: 14,
        registering: 15,
      };
      const overall = activeImportSource === "online"
        ? percent != null
          ? 12 + (percent / 100) * 3
          : onlineFallbackPercent[stage]
        : percent != null
          ? 5 + (percent / 100) * 10
          : localFallbackPercent[stage];

      scheduleProgressUpdate({
        active: true,
        stage: "importing",
        message,
        percent: Math.round(overall),
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
      // 仅在导入流程进行中时更新进度条
      if (!importProgress.active) return;

      // 使用 requestAnimationFrame 批量更新，避免高频状态变更导致掉帧
      if (stage === "preparing") {
        scheduleProgressUpdate({ active: true, stage: "preparing", message: "正在检查依赖和模型…", percent: 18 });
      } else if (stage === "recognizing") {
        // whisper 实时回报了识别进度 → 映射到总进度 25%–85%
        if (percent != null && percent > 0) {
          const overall = 25 + (percent / 100) * 60; // 25% ~ 85%
          scheduleProgressUpdate({
            active: true,
            stage: "recognizing",
            message: `正在离线识别字幕… ${Math.round(percent)}%`,
            percent: Math.round(overall),
          });
        } else {
          scheduleProgressUpdate({ active: true, stage: "recognizing", message: "正在离线识别字幕…", percent: 25 });
        }
      } else if (stage === "writing") {
        scheduleProgressUpdate({ active: true, stage: "recognizing", message: "识别完成，正在写入字幕…", percent: 85 });
      }
    });

    const unAsrCompleted = await asrEvents.onCompleted(async ({ jobId, subtitlePath, detectedLanguage }) => {
      if (activeAsrJobId !== jobId) return;
      activeAsrJobId = undefined;
      isCancellingAsr = false;
      try {
        let finalSubtitlePath = subtitlePath;
        let translationError: string | undefined;
        const shouldTranslateToChinese = !isChineseLanguage(detectedLanguage);
        const mediaIdForSubtitle = pendingSubtitleMediaId;
        if (mediaIdForSubtitle) {
          await backend.updateMediaSubtitle(mediaIdForSubtitle, subtitlePath);
          if (shouldTranslateToChinese) {
            if (importProgress.active) {
              resetScheduledProgressUpdate();
              importProgress = { active: true, stage: "translating", message: "正在生成中文翻译…", percent: 85 };
            }
            try {
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
          translationError
            ? `离线识别完成，但中文字幕生成失败：${translationError}`
            : shouldTranslateToChinese
              ? "离线识别完成，双语字幕已绑定"
              : "离线识别完成，原文字幕已绑定",
          translationError ? "warning" : "success",
        );

        // 整个流水线结束 → 显示成功弹框
        if (importProgress.active) {
          resetScheduledProgressUpdate();
          importProgress = { active: true, stage: "done", message: "全部完成！", percent: 100 };
        }

        // 延迟执行 refreshLibrary + loadSubtitle，让 UI 先完成 100% 进度渲染
        // 避免完成瞬间大量 IPC + DOM 更新挤占渲染帧
        await new Promise((r) => setTimeout(r, 50));

        await refreshLibrary();
        if (mediaIdForSubtitle && currentMediaId === mediaIdForSubtitle) {
          await loadSubtitleFromPath(finalSubtitlePath);
        }

        if (importProgress.active) {
          // 短暂显示 100% 后弹出成功提示
          clearTimeout(importSuccessTimer);
          importSuccessTimer = setTimeout(() => {
            showImportSuccess = true;
            importProgress = { ...IMPORT_IDLE };
            activeImportSource = undefined;
          }, 550);
        }
      } catch (err) {
        console.error(err);
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
      modelsStatusMap = new Map(modelsStatusMap).set(status.modelId, status);
      refreshModelLabels();
      if (showFirstRunOnboarding && onboardingSelectedModelId === status.modelId) {
        onboardingDownloadPercent = 100;
        onboardingDownloadMessage = "模型下载完成，可以开始使用了。";
        onboardingError = undefined;
        onboardingStep = "ready";
      }
      const label = availableModels.find((m) => m.id === status.modelId)?.label ?? status.modelId;
      setStatus(`模型 ${label} 下载完成`, "success");
    });

    const unModelFailed = await modelEvents.onFailed(({ jobId, code, message }) => {
      if (activeModelDownloadJobId && activeModelDownloadJobId !== jobId) return;
      activeModelDownloadJobId = undefined;
      if (showFirstRunOnboarding && onboardingStep === "downloading") {
        onboardingError = `[${code}] ${message}`;
        onboardingDownloadMessage = "模型下载失败，请重试。";
      }
      setStatus(`[${code}] ${message}`, "warning");
    });

    // Load settings
    try {
      settings = await backend.getSettings();
      if (!settings.playlistMode) settings = { ...settings, playlistMode: "sequential" };
      if (!settings.themeMode) settings = { ...settings, themeMode: "dark" };
      player.setPlaybackRate(settings.playbackRate);
      player.setVolume(settings.volume);
      await overlayBridge.updateStyle(settings.overlay);
      if (settings.overlayVisible) await backend.showOverlay();
      showFirstRunOnboarding = !settings.hasCompletedOnboarding;
      onboardingStep = settings.hasCompletedOnboarding ? "ready" : "select-model";
      setStatus("设置已加载", "success");
    } catch (err) {
      console.error(err);
      showFirstRunOnboarding = !DEFAULT_SETTINGS.hasCompletedOnboarding;
      setStatus("读取设置失败，已使用默认配置", "warning");
    }

    // Load library
    try {
      await refreshLibrary();
    } catch (err) {
      console.error(err);
      setStatus("读取素材库失败", "warning");
    }

    // Load model status
    try {
      availableModels = await backend.getAvailableModels();
      const allStatus = await backend.getAllModelsStatus();
      const newMap = new Map<string, ModelStatus>();
      for (const s of allStatus.models) newMap.set(s.modelId, s);
      modelsStatusMap = newMap;
      refreshModelLabels();
    } catch (err) {
      console.error(err);
      modelStatusLabel = "模型状态读取失败";
    }

    if (!showFirstRunOnboarding && !settings.hasSeenMainTour) {
      await openMainTour();
    }

    return () => {
      unlistenLock();
      unlistenAppClose();
      unlistenClose();
      unlistenWindowClose();
      unlistenWindowResized?.();
      window.removeEventListener("resize", handleViewportUpdate);
      document.removeEventListener("scroll", handleViewportUpdate, true);
      unImportProgress();
      unAsrStarted();
      unAsrProgress();
      unAsrCompleted();
      unAsrFailed();
      unModelStarted();
      unModelProgress();
      unModelCompleted();
      unModelFailed();
      clearTimeout(importSuccessTimer);
      resetScheduledProgressUpdate();
    };
  });
</script>

<svelte:window onkeydown={handleKeydown} />

<!-- Hidden audio element -->
<!-- svelte-ignore a11y_media_has_caption -->
<audio bind:this={audioEl} preload="metadata" style="display:none"></audio>

<main class="app-shell">
  <div class="window-drag-bar" class:window-drag-bar-with-controls={useWindowsCustomFrame}>
    <div class="window-drag-region" data-tauri-drag-region>
      {#if useWindowsCustomFrame}
        <span class="window-caption">字幕工作台</span>
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

  <Sidebar
    {activePage}
    onNavigate={setActivePage}
  />

  <section class="content">
    {#if activePage === "import"}
      <div class="page-transition" in:fade={{ duration: 160, delay: 40 }}>
      <ImportPage
        progress={importProgress}
        {importError}
        {importSuccessName}
        showSuccess={showImportSuccess}
        canCancel={Boolean(activeAsrJobId) && importProgress.stage !== "done"}
        {isCancellingAsr}
        onImportMedia={handleImportMedia}
        onImportOnline={handleImportOnlineMedia}
        onCancel={() => { void handleCancelAsr(); }}
        onDismissError={() => { importError = undefined; }}
        onImportSuccessClose={() => { showImportSuccess = false; importSuccessName = undefined; }}
        onGoToResources={() => setActivePage("resources")}
      />
      </div>
    {:else if activePage === "resources"}
      <div class="page-transition" in:fade={{ duration: 160, delay: 40 }}>
      <ResourceListPage
        items={libraryState.mediaItems}
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
        {hasMedia}
        {audioFileLabel}
        {subtitleFileLabel}
        {cueTiming}
        {currentText}
        {currentSecondaryText}
        {subtitleCues}
        playbackRate={settings.playbackRate}
        playlistMode={settings.playlistMode}
        playlist={libraryState.playbackHistory}
        {pendingPlaylistMediaId}
        {currentMediaId}
        volume={settings.volume}
        onTogglePlayback={() => void player.togglePlayback()}
        onToggleCurrentItem={() => void player.togglePlayback()}
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
        onVolumeChange={(volume) => applyVolume(volume)}
        onVolumeCommit={() => { void commitVolume(); }}
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
        isDownloading={Boolean(activeModelDownloadJobId)}
        modelStatusLabel={modelStatusLabel}
        modelPathLabel={modelPathLabel}
        {overlayLocked}
        onOverlayVisibleChange={handleOverlayVisibleChange}
        onOverlayLockToggle={handleOverlayLockToggle}
        onOverlayStyleChange={handleOverlayStyleChange}
        onOverlayStyleCommit={persistSettings}
        onDownloadModel={handleDownloadModel}
        onSelectModel={handleSelectModel}
        onDeleteModel={handleDeleteModel}
        onShortcutChange={(shortcuts) => {
          settings = { ...settings, shortcuts };
        }}
        onShortcutCommit={persistSettings}
        onClearAllCache={handleClearAllCache}
        onDeleteAllModels={handleDeleteAllModels}
        onResetAppData={handleResetAppData}
        onThemeChange={handleThemeChange}
      />
      </div>
    {:else if activePage === "subtitle-editor"}
      <div class="page-transition" in:fade={{ duration: 160, delay: 40 }}>
      <SubtitleEditor
        document={activeSubtitleDocument}
        lastMainPage={lastMainPage}
        onBack={() => setActivePage(lastMainPage)}
        onSave={() => void saveSubtitleEditor()}
        onCueChange={handleCueChange}
      />
      </div>
    {/if}
  </section>
</main>

<ConfirmDialog bind:this={confirmDialog} />

{#if showFirstRunOnboarding}
  <FirstRunOnboarding
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
    onStart={() => { void handleOnboardingStart(); }}
  />
{/if}

{#if showMainTour}
  <MainTourOverlay
    step={MAIN_TOUR_STEPS[mainTourStepIndex]}
    index={mainTourStepIndex}
    total={MAIN_TOUR_STEPS.length}
    targetRect={mainTourTargetRect}
    onSkip={() => { void completeMainTour(); }}
    onNext={() => { void handleMainTourNext(); }}
    onFinish={() => { void completeMainTour(); }}
  />
{/if}
