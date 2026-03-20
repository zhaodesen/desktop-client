<script lang="ts">
  import { onMount } from "svelte";
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { emitTo, listen } from "@tauri-apps/api/event";
  import { open } from "@tauri-apps/plugin-dialog";
  import { OVERLAY_CLOSE_EVENT, OVERLAY_LOCK_EVENT } from "./shared/events";
  import { asrEvents, backend, modelEvents, overlayBridge } from "./shared/tauri";
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
    SubtitleDocument,
  } from "./shared/types";
  import { PlayerController } from "./main/player-controller";
  import { parseSubtitleText } from "./main/subtitle-parser";
  import { SubtitleEngine } from "./main/subtitle-engine";

  import Sidebar from "./lib/Sidebar.svelte";
  import ImportPage from "./lib/ImportPage.svelte";
  import ResourceListPage from "./lib/ResourceListPage.svelte";
  import PlayerPage from "./lib/PlayerPage.svelte";
  import SettingsPage from "./lib/SettingsPage.svelte";
  import SubtitleEditor from "./lib/SubtitleEditor.svelte";
  import ConfirmDialog from "./lib/ConfirmDialog.svelte";

  import "./styles.css";

  /* ── Constants ─────────────────────────────────────────── */

  const DEFAULT_SETTINGS: AppSettings = {
    playbackRate: 1,
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
    selectedModel: "base",
  };

  /* ── State ─────────────────────────────────────────────── */

  let settings = $state<AppSettings>({ ...DEFAULT_SETTINGS, overlay: { ...DEFAULT_SETTINGS.overlay } });
  let libraryState = $state<LibraryState>({ mediaItems: [], playbackHistory: [] });
  let activePage = $state("import");
  let lastMainPage = $state<"import" | "resources" | "playlist" | "settings">("import");

  let currentMediaId = $state<string | undefined>(undefined);
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

  let availableModels = $state<ModelInfo[]>([]);
  let modelsStatusMap = $state<Map<string, ModelStatus>>(new Map());
  let activeSubtitleDocument = $state<SubtitleDocument | undefined>(undefined);

  // Player state (published by PlayerController)
  let snap = $state<PlaybackSnapshot>({ playing: false, currentTimeMs: 0, durationMs: 0, rate: 1 });
  let hasMedia = $state(false);
  let audioFileLabel = $state("未选择素材");
  let subtitleFileLabel = $state("未生成字幕");
  let cueTiming = $state("--:-- ~ --:--");
  let currentText = $state("等待播放");
  let currentSecondaryText = $state("");
  let showRetryAsr = $state(false);

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

  // Internal services (created in onMount)
  let player: PlayerController;
  let subtitleEngine: SubtitleEngine;
  let audioEl: HTMLAudioElement;

  // ConfirmDialog ref
  let confirmDialog: ConfirmDialog;

  /* ── Helpers ────────────────────────────────────────────── */

  function formatDuration(ms: number): string {
    const t = Math.max(0, Math.floor(ms / 1000));
    const h = Math.floor(t / 3600);
    const m = Math.floor((t % 3600) / 60);
    const s = t % 60;
    const mm = String(m).padStart(2, "0");
    const ss = String(s).padStart(2, "0");
    return h > 0 ? `${String(h).padStart(2, "0")}:${mm}:${ss}` : `${mm}:${ss}`;
  }

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

  function getCurrentMedia(): MediaItem | undefined {
    return libraryState.mediaItems.find((i) => i.id === currentMediaId);
  }

  function setActivePage(page: string) {
    if (page === "import" || page === "resources" || page === "playlist" || page === "settings") {
      lastMainPage = page;
    }
    activePage = page;
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

  function renderSubtitle(s: PlaybackSnapshot) {
    const ctx = subtitleEngine.getContext(s.currentTimeMs);
    currentText = ctx.current?.text ?? "当前时间点暂无字幕";
    currentSecondaryText = ctx.current?.secondaryText ?? "";
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
      current: ctx.current,
      next: undefined,
      playback: s,
    });
  }

  async function loadSubtitleFromPath(path: string) {
    const content = await fetch(convertFileSrc(path)).then((r) => r.text());
    const cues = parseSubtitleText(content);
    if (cues.length === 0) throw new Error("未解析出有效字幕");
    subtitleEngine.load(cues);
    subtitleFileLabel = `${path.split(/[\\/]/).pop() ?? "字幕"} · ${cues.length} 句`;
  }

  /* ── Media operations ──────────────────────────────────── */

  async function resetPlaybackUi() {
    subtitleEngine.clear();
    currentMediaId = undefined;
    player.pause();
    audioFileLabel = "未选择素材";
    subtitleFileLabel = "未生成字幕";
    currentText = "等待播放";
    currentSecondaryText = "";
    cueTiming = "--:-- ~ --:--";
    showRetryAsr = false;
    await overlayBridge.clear();
  }

  async function loadMediaById(mediaId: string, record: boolean) {
    const media = libraryState.mediaItems.find((i) => i.id === mediaId);
    if (!media) { setStatus("未找到对应素材", "warning"); return; }

    await player.loadUrl(convertFileSrc(media.audioPath));
    player.setPlaybackRate(settings.playbackRate);
    currentMediaId = media.id;
    audioFileLabel = media.title;
    subtitleEngine.clear();
    subtitleFileLabel = media.subtitlePath ? "正在加载字幕…" : "未生成字幕";
    showRetryAsr = !media.subtitlePath;

    if (media.subtitlePath) {
      try {
        await loadSubtitleFromPath(media.subtitlePath);
        showRetryAsr = false;
      } catch (err) {
        console.error(err);
        subtitleFileLabel = "字幕加载失败";
        showRetryAsr = true;
      }
    }

    renderSubtitle(player.getSnapshot());
    await syncOverlay(player.getSnapshot());

    if (record) {
      await backend.recordPlayback(media.id);
      await refreshLibrary();
    }
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

  async function startAutoAsr(media: MediaItem) {
    try {
      const { jobId } = await backend.startAsrJob({ audioPath: media.audioPath });
      activeAsrJobId = jobId;
      pendingSubtitleMediaId = media.id;
      setStatus("素材已导入，正在离线生成字幕…", "neutral");
    } catch (err) {
      console.error(err);
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

  async function handleImportMedia() {
    const selected = await open({
      multiple: false,
      filters: [{
        name: "媒体",
        extensions: ["mp3", "wav", "m4a", "aac", "flac", "ogg", "opus", "mp4", "m4v", "mov", "webm", "mkv", "avi"],
      }],
    });
    if (!selected || Array.isArray(selected)) return;

    importError = undefined;
    importProgress = {
      active: true,
      stage: "importing",
      message: "正在导入媒体文件…",
      percent: 5,
    };
    try {
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
      importProgress = { ...IMPORT_IDLE };
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
      showRetryAsr = false;
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

  /* ── Model handlers ────────────────────────────────────── */

  async function handleDownloadModel(modelId: string) {
    try {
      const { jobId } = await backend.downloadModel(modelId);
      activeModelDownloadJobId = jobId;
      const label = availableModels.find((m) => m.id === modelId)?.label ?? modelId;
      setStatus(`模型 ${label} 开始下载`, "neutral");
    } catch (err) {
      console.error(err);
      setStatus("启动模型下载失败", "warning");
    }
  }

  async function handleSelectModel(modelId: string) {
    settings = { ...settings, selectedModel: modelId };
    await persistSettings();
    try {
      const s = await backend.getModelStatus(modelId);
      modelsStatusMap = new Map(modelsStatusMap).set(modelId, s);
    } catch { /* ignore */ }
    refreshModelLabels();
    const label = availableModels.find((m) => m.id === modelId)?.label ?? modelId;
    setStatus(`已切换为 ${label} 模型`, "success");
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
    switch (e.code) {
      case "Space":
        e.preventDefault();
        if (hasMedia) void player.togglePlayback();
        break;
      case "ArrowLeft":
        e.preventDefault();
        player.seek(Math.max(0, player.getSnapshot().currentTimeMs - 5000));
        break;
      case "ArrowRight":
        e.preventDefault();
        player.seek(player.getSnapshot().currentTimeMs + 5000);
        break;
      case "ArrowUp":
        e.preventDefault();
        player.seek(Math.max(0, player.getSnapshot().currentTimeMs - 1000));
        break;
      case "ArrowDown":
        e.preventDefault();
        player.seek(player.getSnapshot().currentTimeMs + 1000);
        break;
    }
  }

  /* ── onMount (init) ────────────────────────────────────── */

  onMount(async () => {
    // Init services
    subtitleEngine = new SubtitleEngine();
    player = new PlayerController(audioEl);

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

    // ASR events
    const unAsrStarted = await asrEvents.onStarted(({ jobId }) => {
      activeAsrJobId = jobId;
      // 不再更新已移除的侧边栏状态，只记录 jobId
    });

    const unAsrProgress = await asrEvents.onProgress(({ jobId, stage, message }) => {
      if (activeAsrJobId && activeAsrJobId !== jobId) return;
      console.debug("[ASR]", stage, message);
      // 仅在导入流程进行中时更新进度条（低频事件，不会造成卡顿）
      if (importProgress.active) {
        const stageMap: Record<string, { label: string; percent: number }> = {
          preparing: { label: "正在检查依赖和模型…", percent: 18 },
          recognizing: { label: "正在离线识别字幕…", percent: 40 },
          writing: { label: "识别完成，正在写入字幕…", percent: 80 },
        };
        const info = stageMap[stage];
        if (info) {
          importProgress = {
            active: true,
            stage: stage as "preparing" | "recognizing",
            message: info.label,
            percent: info.percent,
          };
        }
      }
    });

    const unAsrCompleted = await asrEvents.onCompleted(async ({ jobId, subtitlePath }) => {
      if (activeAsrJobId !== jobId) return;
      activeAsrJobId = undefined;
      try {
        let finalSubtitlePath = subtitlePath;
        let translationError: string | undefined;
        if (pendingSubtitleMediaId) {
          await backend.updateMediaSubtitle(pendingSubtitleMediaId, subtitlePath);
          // 进入翻译阶段
          if (importProgress.active) {
            importProgress = { active: true, stage: "translating", message: "正在生成中文翻译…", percent: 85 };
          }
          try {
            const translated = await backend.translateMediaSubtitle(pendingSubtitleMediaId);
            finalSubtitlePath = translated.subtitlePath;
          } catch (err) {
            console.error(err);
            translationError = formatError(err);
          }
        }
        await refreshLibrary();
        if (pendingSubtitleMediaId && currentMediaId === pendingSubtitleMediaId) {
          await loadSubtitleFromPath(finalSubtitlePath);
          showRetryAsr = false;
        }
        setStatus(
          translationError
            ? `离线识别完成，但中文字幕生成失败：${translationError}`
            : "离线识别完成，双语字幕已绑定",
          translationError ? "warning" : "success",
        );
        // 整个流水线结束 → 显示成功弹框
        if (importProgress.active) {
          importProgress = { active: true, stage: "done", message: "全部完成！", percent: 100 };
          // 短暂显示 100% 后弹出成功提示
          setTimeout(() => {
            showImportSuccess = true;
            importProgress = { ...IMPORT_IDLE };
          }, 600);
        }
      } catch (err) {
        console.error(err);
        setStatus("识别完成，但字幕绑定失败", "warning");
        importProgress = { ...IMPORT_IDLE };
      } finally {
        pendingSubtitleMediaId = undefined;
      }
    });

    const unAsrFailed = await asrEvents.onFailed(({ jobId, code, message }) => {
      if (activeAsrJobId && activeAsrJobId !== jobId) return;
      activeAsrJobId = undefined;
      pendingSubtitleMediaId = undefined;
      showRetryAsr = true;
      setStatus(`[${code}] ${message}`, "warning");
      // 识别失败时重置进度条并显示错误
      if (importProgress.active) {
        importProgress = { ...IMPORT_IDLE };
        importError = `字幕生成失败: [${code}] ${message}`;
      }
    });

    // Model download events
    const unModelStarted = await modelEvents.onStarted(({ jobId }) => {
      activeModelDownloadJobId = jobId;
    });

    const unModelProgress = await modelEvents.onProgress(({ jobId, message }) => {
      if (activeModelDownloadJobId && activeModelDownloadJobId !== jobId) return;
      console.debug("[Model Download]", message);
    });

    const unModelCompleted = await modelEvents.onCompleted(({ jobId, status }) => {
      if (activeModelDownloadJobId && activeModelDownloadJobId !== jobId) return;
      activeModelDownloadJobId = undefined;
      modelsStatusMap = new Map(modelsStatusMap).set(status.modelId, status);
      refreshModelLabels();
      const label = availableModels.find((m) => m.id === status.modelId)?.label ?? status.modelId;
      setStatus(`模型 ${label} 下载完成`, "success");
    });

    const unModelFailed = await modelEvents.onFailed(({ jobId, code, message }) => {
      if (activeModelDownloadJobId && activeModelDownloadJobId !== jobId) return;
      activeModelDownloadJobId = undefined;
      setStatus(`[${code}] ${message}`, "warning");
    });

    // Load settings
    try {
      settings = await backend.getSettings();
      if (!settings.playlistMode) settings = { ...settings, playlistMode: "sequential" };
      player.setPlaybackRate(settings.playbackRate);
      await overlayBridge.updateStyle(settings.overlay);
      if (settings.overlayVisible) await backend.showOverlay();
      setStatus("设置已加载", "success");
    } catch (err) {
      console.error(err);
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

    return () => {
      unlistenLock();
      unlistenClose();
      unAsrStarted();
      unAsrProgress();
      unAsrCompleted();
      unAsrFailed();
      unModelStarted();
      unModelProgress();
      unModelCompleted();
      unModelFailed();
    };
  });
</script>

<svelte:window onkeydown={handleKeydown} />

<!-- Hidden audio element -->
<!-- svelte-ignore a11y_media_has_caption -->
<audio bind:this={audioEl} preload="metadata" style="display:none"></audio>

<main class="app-shell">
  <!-- 专用拖拽条：横跨两列，data-tauri-drag-region 是 Tauri 2 官方机制 -->
  <div class="window-drag-bar" data-tauri-drag-region></div>

  <Sidebar
    {activePage}
    onNavigate={setActivePage}
  />

  <section class="content">
    {#if activePage === "import"}
      <ImportPage
        progress={importProgress}
        {importError}
        {importSuccessName}
        showSuccess={showImportSuccess}
        onImportMedia={handleImportMedia}
        onDismissError={() => { importError = undefined; }}
        onImportSuccessClose={() => { showImportSuccess = false; importSuccessName = undefined; }}
        onGoToResources={() => setActivePage("resources")}
      />
    {:else if activePage === "resources"}
      <ResourceListPage
        items={libraryState.mediaItems}
        onEditSubtitle={(id) => void openSubtitleEditor(id)}
        onDeleteMedia={(id) => void deleteMediaById(id)}
        onAddToPlaylist={(id) => void handleAddToPlaylist(id)}
        onImportMedia={handleImportMedia}
      />
    {:else if activePage === "playlist"}
      <PlayerPage
        {snap}
        {hasMedia}
        {audioFileLabel}
        {subtitleFileLabel}
        {cueTiming}
        {currentText}
        {currentSecondaryText}
        playbackRate={settings.playbackRate}
        playlistMode={settings.playlistMode}
        {showRetryAsr}
        playlist={libraryState.playbackHistory}
        {currentMediaId}
        onTogglePlayback={() => void player.togglePlayback()}
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
        onRetryAsr={() => { const m = getCurrentMedia(); if (m) void startAutoAsr(m); }}
        onPlayItem={(id) => { void loadMediaById(id, true); }}
      />
    {:else if activePage === "settings"}
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
        onClearAllCache={handleClearAllCache}
        onDeleteAllModels={handleDeleteAllModels}
        onResetAppData={handleResetAppData}
      />
    {:else if activePage === "subtitle-editor"}
      <SubtitleEditor
        document={activeSubtitleDocument}
        lastMainPage={lastMainPage}
        onBack={() => setActivePage(lastMainPage)}
        onSave={() => void saveSubtitleEditor()}
        onCueChange={handleCueChange}
      />
    {/if}
  </section>
</main>

<ConfirmDialog bind:this={confirmDialog} />
