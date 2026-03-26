import { convertFileSrc } from "@tauri-apps/api/core";
import { emitTo, listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { OVERLAY_CLOSE_EVENT, OVERLAY_LOCK_EVENT } from "../shared/events";
import { asrEvents, backend, modelEvents, overlayBridge } from "../shared/tauri";
import type {
  AppSettings,
  CleanupResult,
  DefaultModelStatus,
  LibraryState,
  MediaItem,
  ModelInfo,
  ModelStatus,
  OverlaySettings,
  PlaybackSnapshot,
  PlaylistMode,
  SubtitleCue,
  SubtitleContext,
  SubtitleDocument,
} from "../shared/types";
import { formatDuration } from "../shared/utils";
import { PlayerController } from "./player-controller";
import { parseSubtitleText } from "./subtitle-parser";
import { SubtitleEngine } from "./subtitle-engine";

/* ============================================================
   Helpers
   ============================================================ */

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
  selectedModel: "base",
  hasCompletedOnboarding: false,
  hasSeenMainTour: false,
  themeMode: "dark",
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
};

function queryElement<T extends Element>(selector: string): T {
  const element = document.querySelector<T>(selector);
  if (!element) throw new Error(`找不到元素: ${selector}`);
  return element;
}

function formatTimestamp(ts: number): string {
  return new Date(ts).toLocaleString("zh-CN", { month: "2-digit", day: "2-digit", hour: "2-digit", minute: "2-digit" });
}

function escapeHtml(v: string): string {
  return v.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;").replace(/'/g, "&#39;");
}

function isChineseLanguage(code?: string): boolean {
  return code?.trim().toLowerCase().startsWith("zh") ?? false;
}

/* ============================================================
   Confirm dialog
   ============================================================ */

function showConfirm(title: string, message: string): Promise<boolean> {
  const dialog = queryElement<HTMLDialogElement>("#confirm-dialog");
  queryElement("#confirm-title").textContent = title;
  queryElement("#confirm-message").textContent = message;
  dialog.showModal();

  return new Promise((resolve) => {
    const cancel = queryElement<HTMLButtonElement>("#confirm-cancel");
    const ok = queryElement<HTMLButtonElement>("#confirm-ok");

    const cleanup = () => {
      cancel.removeEventListener("click", onCancel);
      ok.removeEventListener("click", onOk);
      dialog.close();
    };

    const onCancel = () => { cleanup(); resolve(false); };
    const onOk = () => { cleanup(); resolve(true); };

    cancel.addEventListener("click", onCancel);
    ok.addEventListener("click", onOk);
  });
}

/* ============================================================
   DOM references
   ============================================================ */

type DomRefs = {
  audioElement: HTMLAudioElement;
  importMediaButton: HTMLButtonElement;
  importSubtitleButton: HTMLButtonElement;
  tabTriggers: NodeListOf<HTMLButtonElement>;
  tabPanels: NodeListOf<HTMLElement>;
  statusBadge: HTMLElement;
  statusBadgeLabel: HTMLElement;
  statusText: HTMLElement;
  libraryCountLabel: HTMLElement;
  mediaLibraryList: HTMLElement;
  playlistCountLabel: HTMLElement;
  playbackHistoryList: HTMLElement;
  audioFileLabel: HTMLElement;
  subtitleFileLabel: HTMLElement;
  cueTiming: HTMLElement;
  playToggleButton: HTMLButtonElement;
  playIcon: HTMLElement;
  pauseIcon: HTMLElement;
  progressInput: HTMLInputElement;
  transportTime: HTMLElement;
  durationMeta: HTMLElement;
  currentText: HTMLElement;
  currentSecondaryText: HTMLElement;
  playbackRateSelect: HTMLSelectElement;
  playlistModeSelect: HTMLSelectElement;
  retryAsrButton: HTMLButtonElement;
  subtitleEditorTitle: HTMLElement;
  subtitleEditorList: HTMLElement;
  subtitleEditorBackButton: HTMLButtonElement;
  subtitleEditorSaveButton: HTMLButtonElement;
  overlayVisibleCheckbox: HTMLInputElement;
  overlayPositionSelect: HTMLSelectElement;
  overlayColorInput: HTMLInputElement;
  strokeColorInput: HTMLInputElement;
  secondaryColorInput: HTMLInputElement;
  secondaryStrokeColorInput: HTMLInputElement;
  fontSizeInput: HTMLInputElement;
  opacityInput: HTMLInputElement;
  fontSizeValue: HTMLElement;
  opacityValue: HTMLElement;
  modelList: HTMLElement;
  modelStatusLabel: HTMLElement;
  modelPathLabel: HTMLElement;
  clearSubtitlesButton: HTMLButtonElement;
  clearAudioCacheButton: HTMLButtonElement;
  deleteModelButton: HTMLButtonElement;
  resetAppDataButton: HTMLButtonElement;
  overlayLockToggle: HTMLButtonElement;
};

function getDomRefs(): DomRefs {
  return {
    audioElement: queryElement("#audio-element"),
    importMediaButton: queryElement("#import-audio-button"),
    importSubtitleButton: queryElement("#import-subtitle-button"),
    tabTriggers: document.querySelectorAll<HTMLButtonElement>("[data-tab-trigger]"),
    tabPanels: document.querySelectorAll<HTMLElement>("[data-tab-panel]"),
    statusBadge: queryElement("#status-badge"),
    statusBadgeLabel: queryElement("#status-badge-label"),
    statusText: queryElement("#status-text"),
    libraryCountLabel: queryElement("#library-count-label"),
    mediaLibraryList: queryElement("#media-library-list"),
    playlistCountLabel: queryElement("#playlist-count-label"),
    playbackHistoryList: queryElement("#playback-history-list"),
    audioFileLabel: queryElement("#audio-file-label"),
    subtitleFileLabel: queryElement("#subtitle-file-label"),
    cueTiming: queryElement("#cue-timing"),
    playToggleButton: queryElement("#play-toggle-button"),
    playIcon: queryElement("#play-icon"),
    pauseIcon: queryElement("#pause-icon"),
    progressInput: queryElement("#progress-input"),
    transportTime: queryElement("#transport-time"),
    durationMeta: queryElement("#duration-meta"),
    currentText: queryElement("#current-text"),
    currentSecondaryText: queryElement("#current-secondary-text"),
    playbackRateSelect: queryElement("#playback-rate-select"),
    playlistModeSelect: queryElement("#playlist-mode-select"),
    retryAsrButton: queryElement("#retry-asr-button"),
    subtitleEditorTitle: queryElement("#subtitle-editor-title"),
    subtitleEditorList: queryElement("#subtitle-editor-list"),
    subtitleEditorBackButton: queryElement("#subtitle-editor-back-button"),
    subtitleEditorSaveButton: queryElement("#subtitle-editor-save-button"),
    overlayVisibleCheckbox: queryElement("#overlay-visible-checkbox"),
    overlayPositionSelect: queryElement("#overlay-position-select"),
    overlayColorInput: queryElement("#overlay-color-input"),
    strokeColorInput: queryElement("#stroke-color-input"),
    secondaryColorInput: queryElement("#secondary-color-input"),
    secondaryStrokeColorInput: queryElement("#secondary-stroke-color-input"),
    fontSizeInput: queryElement("#font-size-input"),
    opacityInput: queryElement("#opacity-input"),
    fontSizeValue: queryElement("#font-size-value"),
    opacityValue: queryElement("#opacity-value"),
    modelList: queryElement("#model-list"),
    modelStatusLabel: queryElement("#model-status-label"),
    modelPathLabel: queryElement("#model-path-label"),
    clearSubtitlesButton: queryElement("#clear-subtitles-button"),
    clearAudioCacheButton: queryElement("#clear-audio-cache-button"),
    deleteModelButton: queryElement("#delete-model-button"),
    resetAppDataButton: queryElement("#reset-app-data-button"),
    overlayLockToggle: queryElement("#overlay-lock-toggle"),
  };
}

/* ============================================================
   Bootstrap
   ============================================================ */

export async function bootstrapMainApp(): Promise<void> {
  const dom = getDomRefs();
  const subtitleEngine = new SubtitleEngine();
  const player = new PlayerController(dom.audioElement);

  // ---- State ----
  let settings: AppSettings = { ...DEFAULT_SETTINGS };
  let libraryState: LibraryState = { mediaItems: [], playbackHistory: [] };
  let currentMediaId: string | undefined;
  let activeAsrJobId: string | undefined;
  let activeModelDownloadJobId: string | undefined;
  let pendingSubtitleMediaId: string | undefined;
  let modelStatus: DefaultModelStatus | undefined;
  let availableModels: ModelInfo[] = [];
  let modelsStatusMap: Map<string, ModelStatus> = new Map();
  let activeSubtitleDocument: SubtitleDocument | undefined;
  let lastMainPage: "library" | "player" | "settings" = "library";
  let overlayLocked = false; // tracks overlay window lock state

  // Store unlisten handles for cleanup
  const unlisteners: Array<() => void> = [];

  // ---- Status ----
  type StatusTone = "neutral" | "success" | "warning";

  const setStatus = (text: string, tone: StatusTone = "neutral") => {
    dom.statusText.textContent = text;
    dom.statusBadge.dataset.tone = tone;
    dom.statusBadgeLabel.textContent = activeAsrJobId || activeModelDownloadJobId
      ? "运行中"
      : tone === "success" ? "完成" : tone === "warning" ? "注意" : "就绪";
  };

  const formatErrorMessage = (err: unknown) => {
    if (err instanceof Error) return err.message;
    if (typeof err === "string") return err;
    return "未知错误";
  };

  // ---- Getters ----
  const getCurrentMedia = (): MediaItem | undefined =>
    libraryState.mediaItems.find((item) => item.id === currentMediaId);

  const getSubtitleContext = (snap: PlaybackSnapshot): SubtitleContext =>
    subtitleEngine.getContext(snap.currentTimeMs);

  const fmtCleanup = (r: CleanupResult) =>
    `已删除 ${r.deletedFiles} 个文件，${r.deletedDirs} 个目录`;

  const setActivePage = (page: string) => {
    if (page === "library" || page === "player" || page === "settings") {
      lastMainPage = page;
    }

    dom.tabTriggers.forEach((trigger) => {
      trigger.dataset.active = String(trigger.dataset.tabTrigger === page);
    });
    dom.tabPanels.forEach((panel) => {
      panel.dataset.active = String(panel.dataset.tabPanel === page);
    });
  };

  const formatCueEditorTime = (cue: SubtitleCue) =>
    `${formatDuration(cue.startMs)} - ${formatDuration(cue.endMs)}`;

  // ---- Overlay ----
  const applyOverlayPreview = (ov: OverlaySettings) => {
    dom.overlayVisibleCheckbox.checked = settings.overlayVisible;
    dom.overlayPositionSelect.value = ov.position;
    dom.overlayColorInput.value = ov.color;
    dom.strokeColorInput.value = ov.strokeColor;
    dom.secondaryColorInput.value = ov.secondaryColor;
    dom.secondaryStrokeColorInput.value = ov.secondaryStrokeColor;
    dom.fontSizeInput.value = String(ov.fontSize);
    dom.opacityInput.value = String(Math.round(ov.opacity * 100));
    dom.fontSizeValue.textContent = `${Math.round(ov.fontSize)}px`;
    dom.opacityValue.textContent = `${Math.round(ov.opacity * 100)}%`;
  };

  const syncOverlay = async (snap: PlaybackSnapshot) => {
    const media = getCurrentMedia();
    const ctx = getSubtitleContext(snap);
    await overlayBridge.render({
      fileLabel: media?.title,
      previous: undefined,
      current: ctx.current,
      next: undefined,
      playback: snap,
    });
  };

  const persistSettings = async () => {
    settings = await backend.updateSettings(settings);
    applyOverlayPreview(settings.overlay);
    await overlayBridge.updateStyle(settings.overlay);
  };

  // ---- Model UI ----
  const updateModelUi = () => {
    const selected = settings.selectedModel || "base";

    // Update top-level status label
    const selectedStatus = modelsStatusMap.get(selected);
    if (selectedStatus) {
      dom.modelStatusLabel.textContent = selectedStatus.installed
        ? `当前选用: ${availableModels.find((m) => m.id === selected)?.label ?? selected} · 已就绪`
        : `当前选用: ${availableModels.find((m) => m.id === selected)?.label ?? selected} · 未安装`;
      dom.modelPathLabel.textContent = selectedStatus.path ?? "模型未下载";
    } else {
      dom.modelStatusLabel.textContent = "正在检查模型状态…";
      dom.modelPathLabel.textContent = "";
    }

    // Render model list
    dom.modelList.innerHTML = availableModels
      .map((info) => {
        const status = modelsStatusMap.get(info.id);
        const installed = status?.installed ?? false;
        const isSelected = info.id === selected;
        const isDownloading = Boolean(activeModelDownloadJobId);
        return `<div class="model-item" data-model-id="${info.id}" data-selected="${isSelected}">
          <div class="model-item-info">
            <div class="model-item-title">
              ${escapeHtml(info.label)}
              ${isSelected ? '<span class="badge">当前</span>' : ""}
              ${installed ? '<span class="badge badge-installed">已安装</span>' : ""}
            </div>
            <div class="model-item-desc">${escapeHtml(info.description)}</div>
          </div>
          <div class="model-item-actions">
            ${
              !installed
                ? `<button class="btn btn-sm btn-outline" data-action="download-model" ${isDownloading ? "disabled" : ""}>下载</button>`
                : !isSelected
                  ? `<button class="btn btn-sm" data-action="select-model">选用</button>`
                  : ""
            }
            ${installed ? `<button class="btn btn-sm btn-danger" data-action="delete-model">删除</button>` : ""}
          </div>
        </div>`;
      })
      .join("");
  };

  // Event delegation for model list actions
  dom.modelList.addEventListener("click", async (e) => {
    const btn = (e.target as HTMLElement).closest<HTMLButtonElement>("button[data-action]");
    if (!btn) return;
    const item = btn.closest<HTMLElement>(".model-item");
    const modelId = item?.dataset.modelId;
    if (!modelId) return;
    const action = btn.dataset.action;

    if (action === "download-model") {
      try {
        const { jobId } = await backend.downloadModel(modelId);
        activeModelDownloadJobId = jobId;
        updateModelUi();
        const label = availableModels.find((m) => m.id === modelId)?.label ?? modelId;
        setStatus(`模型 ${label} 开始下载`, "neutral");
      } catch (err) {
        console.error(err);
        setStatus("启动模型下载失败", "warning");
      }
    } else if (action === "select-model") {
      settings.selectedModel = modelId;
      await persistSettings();
      // Refresh status for the selected model
      try {
        modelStatus = await backend.getModelStatus(modelId);
        modelsStatusMap.set(modelId, modelStatus);
      } catch { /* ignore */ }
      updateModelUi();
      const label = availableModels.find((m) => m.id === modelId)?.label ?? modelId;
      setStatus(`已切换为 ${label} 模型`, "success");
    } else if (action === "delete-model") {
      const label = availableModels.find((m) => m.id === modelId)?.label ?? modelId;
      if (!await showConfirm("删除模型", `确定删除模型 ${label} 吗？删除后需要重新下载。`)) return;
      try {
        const r = await backend.deleteModel(modelId);
        // Refresh status
        const s = await backend.getModelStatus(modelId);
        modelsStatusMap.set(modelId, s);
        if (modelId === settings.selectedModel) modelStatus = s;
        updateModelUi();
        setStatus(`模型 ${label} 已删除，${fmtCleanup(r)}`, "success");
      } catch (err) {
        console.error(err);
        setStatus(`删除模型 ${label} 失败`, "warning");
      }
    }
  });

  // ---- Transport ----
  const renderTransport = (snap: PlaybackSnapshot) => {
    const dur = Math.max(snap.durationMs, 0);
    dom.playToggleButton.disabled = !player.hasMedia();
    dom.progressInput.disabled = !player.hasMedia();
    dom.progressInput.max = String(Math.max(dur, 1));
    dom.progressInput.value = String(Math.min(snap.currentTimeMs, dur || snap.currentTimeMs));

    dom.playIcon.style.display = snap.playing ? "none" : "";
    dom.pauseIcon.style.display = snap.playing ? "" : "none";

    dom.transportTime.textContent = `${formatDuration(snap.currentTimeMs)} / ${formatDuration(dur)}`;
    dom.durationMeta.textContent = player.hasMedia()
      ? `时长 ${formatDuration(dur)} · ${snap.rate.toFixed(2)}x`
      : "等待导入媒体";
  };

  const renderSubtitle = (snap: PlaybackSnapshot) => {
    const { current } = getSubtitleContext(snap);
    dom.currentText.textContent = current?.text ?? "当前时间点暂无字幕";
    dom.currentSecondaryText.textContent = current?.secondaryText ?? "";
    dom.cueTiming.textContent = current
      ? `${formatDuration(current.startMs)} ~ ${formatDuration(current.endMs)}`
      : "--:-- ~ --:--";
  };

  // ---- Library rendering (event delegation) ----
  const renderLibrary = () => {
    dom.libraryCountLabel.textContent = String(libraryState.mediaItems.length);
    if (libraryState.mediaItems.length === 0) {
      dom.mediaLibraryList.className = "list empty-state";
      dom.mediaLibraryList.innerHTML = `<div class="empty-content">
        <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" opacity="0.4"><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/></svg>
        <span>还没有导入任何素材</span>
      </div>`;
      return;
    }

    dom.mediaLibraryList.className = "list";
    dom.mediaLibraryList.innerHTML = libraryState.mediaItems
      .map((item) => {
        const sub = item.subtitlePath ? "已生成字幕" : "待生成字幕";
        const kind = item.sourceKind === "video" ? "视频" : "音频";
        return `<div class="list-item" data-media-id="${item.id}">
          <div class="list-item-info">
            <div class="list-item-title">${escapeHtml(item.title)}</div>
            <div class="list-item-meta">
              <span>${kind}</span><span>${sub}</span><span>${formatTimestamp(item.importedAt)}</span>
            </div>
          </div>
          <div class="list-item-actions">
            <button class="btn btn-sm" data-action="play-media">播放</button>
            <button class="btn btn-sm" data-action="edit-subtitle" ${item.subtitlePath ? "" : "disabled"}>编辑字幕</button>
            <button class="btn btn-sm btn-danger" data-action="delete-media">删除</button>
          </div>
        </div>`;
      })
      .join("");
  };

  const renderPlaylist = () => {
    dom.playlistCountLabel.textContent = String(libraryState.playbackHistory.length);
    if (libraryState.playbackHistory.length === 0) {
      dom.playbackHistoryList.className = "list empty-state";
      dom.playbackHistoryList.innerHTML = `<div class="empty-content">
        <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" opacity="0.4"><circle cx="12" cy="12" r="10"/><polyline points="12 6 12 12 16 14"/></svg>
        <span>还没有播放记录</span>
      </div>`;
      return;
    }

    dom.playbackHistoryList.className = "list";
    dom.playbackHistoryList.innerHTML = libraryState.playbackHistory
      .map((e) => `<div class="list-item" data-media-id="${e.mediaId}">
          <div class="list-item-info">
            <div class="list-item-title">${escapeHtml(e.title)}</div>
            <div class="list-item-meta">
              <span>播放 ${e.playCount} 次</span><span>${formatTimestamp(e.playedAt)}</span><span>${e.subtitlePath ? "有字幕" : "无字幕"}</span>
            </div>
          </div>
          <div class="list-item-actions">
            <button class="btn btn-sm" data-action="play-history">播放</button>
          </div>
        </div>`)
      .join("");
  };

  const refreshLibrary = async () => {
    libraryState = await backend.getLibraryState();
    renderLibrary();
    renderPlaylist();
  };

  // ---- Media operations ----
  const resetPlaybackUi = async () => {
    subtitleEngine.clear();
    currentMediaId = undefined;
    dom.audioElement.pause();
    dom.audioElement.removeAttribute("src");
    dom.audioElement.load();
    dom.audioFileLabel.textContent = "未选择素材";
    dom.subtitleFileLabel.textContent = "未生成字幕";
    dom.currentText.textContent = "等待播放";
    dom.currentSecondaryText.textContent = "";
    dom.cueTiming.textContent = "--:-- ~ --:--";
    dom.retryAsrButton.style.display = "none";
    renderTransport(player.getSnapshot());
    await overlayBridge.clear();
  };

  const loadSubtitleFromPath = async (path: string) => {
    const content = await fetch(convertFileSrc(path)).then((r) => r.text());
    const cues = parseSubtitleText(content);
    if (cues.length === 0) throw new Error("未解析出有效字幕");
    subtitleEngine.load(cues);
    dom.subtitleFileLabel.textContent = `${path.split(/[\\/]/).pop() ?? "字幕"} · ${cues.length} 句`;
  };

  const renderSubtitleEditor = () => {
    const document = activeSubtitleDocument;
    if (!document) {
      dom.subtitleEditorTitle.textContent = "选择素材后可在这里校对原文和中文字幕";
      dom.subtitleEditorList.className = "subtitle-editor-list empty-state";
      dom.subtitleEditorList.innerHTML = `<div class="empty-content">
        <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" opacity="0.4"><path d="M4 19.5A2.5 2.5 0 0 1 6.5 17H20"/><path d="M6.5 2H20v20H6.5A2.5 2.5 0 0 1 4 19.5v-15A2.5 2.5 0 0 1 6.5 2z"/></svg>
        <span>字幕详情会显示在这里</span>
      </div>`;
      return;
    }

    dom.subtitleEditorTitle.textContent = `${document.title} · ${document.cues.length} 条字幕`;
    dom.subtitleEditorList.className = "subtitle-editor-list";
    dom.subtitleEditorList.innerHTML = document.cues
      .map((cue, index) => `<div class="subtitle-editor-row" data-cue-index="${index}">
        <div class="subtitle-editor-time">${formatCueEditorTime(cue)}</div>
        <div class="subtitle-editor-field">
          <label>原文</label>
          <textarea data-field="text">${escapeHtml(cue.text)}</textarea>
        </div>
        <div class="subtitle-editor-field">
          <label>中文字幕</label>
          <textarea data-field="secondaryText">${escapeHtml(cue.secondaryText ?? "")}</textarea>
        </div>
      </div>`)
      .join("");
  };

  const openSubtitleEditor = async (mediaId: string) => {
    try {
      activeSubtitleDocument = await backend.getSubtitleDocument(mediaId);
      renderSubtitleEditor();
      setActivePage("subtitle-editor");
    } catch (err) {
      console.error(err);
      setStatus(formatErrorMessage(err), "warning");
    }
  };

  const syncEditedSubtitleIfNeeded = async (document: SubtitleDocument) => {
    if (currentMediaId !== document.mediaId) return;
    await loadSubtitleFromPath(document.subtitlePath);
    renderSubtitle(player.getSnapshot());
    await syncOverlay(player.getSnapshot());
  };

  const saveSubtitleEditor = async () => {
    if (!activeSubtitleDocument) {
      setStatus("没有可保存的字幕内容", "warning");
      return;
    }

    const saved = await backend.saveSubtitleDocument(
      activeSubtitleDocument.mediaId,
      activeSubtitleDocument.cues,
    );
    activeSubtitleDocument = saved;
    renderSubtitleEditor();
    await refreshLibrary();
    await syncEditedSubtitleIfNeeded(saved);
    setStatus("字幕校对已保存", "success");
  };

  const loadMediaById = async (mediaId: string, record: boolean) => {
    const media = libraryState.mediaItems.find((i) => i.id === mediaId);
    if (!media) { setStatus("未找到对应素材", "warning"); return; }

    await player.loadUrl(convertFileSrc(media.audioPath));
    player.setPlaybackRate(settings.playbackRate);
    currentMediaId = media.id;
    dom.audioFileLabel.textContent = media.title;
    subtitleEngine.clear();
    dom.subtitleFileLabel.textContent = media.subtitlePath ? "正在加载字幕…" : "未生成字幕";

    // Show retry button if no subtitle
    dom.retryAsrButton.style.display = media.subtitlePath ? "none" : "";

    if (media.subtitlePath) {
      try {
        await loadSubtitleFromPath(media.subtitlePath);
        dom.retryAsrButton.style.display = "none";
      } catch (err) {
        console.error(err);
        dom.subtitleFileLabel.textContent = "字幕加载失败";
        dom.retryAsrButton.style.display = "";
      }
    }

    renderSubtitle(player.getSnapshot());
    renderTransport(player.getSnapshot());
    await syncOverlay(player.getSnapshot());

    if (record) {
      await backend.recordPlayback(media.id);
      await refreshLibrary();
    }
  };

  const deleteMediaById = async (mediaId: string) => {
    const ok = await showConfirm("删除素材", "确定要删除该素材及其字幕吗？此操作不可逆。");
    if (!ok) return;

    await backend.deleteMedia(mediaId);
    if (currentMediaId === mediaId) await resetPlaybackUi();
    await refreshLibrary();
    setStatus("素材已删除", "success");
  };

  const startAutoAsr = async (media: MediaItem) => {
    try {
      const { jobId } = await backend.startAsrJob({ audioPath: media.audioPath });
      activeAsrJobId = jobId;
      pendingSubtitleMediaId = media.id;
      setStatus("素材已导入，正在离线生成字幕…", "neutral");
    } catch (err) {
      console.error(err);
      setStatus(formatErrorMessage(err), "warning");
    }
  };

  /* ===========================================================
     Event bindings — Tabs
     =========================================================== */

  dom.tabTriggers.forEach((trigger) => {
    trigger.addEventListener("click", () => {
      const tab = trigger.dataset.tabTrigger;
      if (tab) setActivePage(tab);
    });
  });

  /* ===========================================================
     Event bindings — Library (event delegation)
     =========================================================== */

  dom.mediaLibraryList.addEventListener("click", (e) => {
    const btn = (e.target as HTMLElement).closest<HTMLElement>("[data-action]");
    if (!btn) return;
    const item = btn.closest<HTMLElement>("[data-media-id]");
    const mediaId = item?.dataset.mediaId;
    if (!mediaId) return;

    if (btn.dataset.action === "play-media") void loadMediaById(mediaId, true);
    if (btn.dataset.action === "edit-subtitle") void openSubtitleEditor(mediaId);
    if (btn.dataset.action === "delete-media") void deleteMediaById(mediaId);
  });

  dom.playbackHistoryList.addEventListener("click", (e) => {
    const btn = (e.target as HTMLElement).closest<HTMLElement>("[data-action]");
    if (!btn) return;
    const item = btn.closest<HTMLElement>("[data-media-id]");
    const mediaId = item?.dataset.mediaId;
    if (!mediaId) return;

    if (btn.dataset.action === "play-history") void loadMediaById(mediaId, true);
  });

  /* ===========================================================
     Event bindings — Import
     =========================================================== */

  dom.importMediaButton.addEventListener("click", async () => {
    const selected = await open({
      multiple: false,
      filters: [{
        name: "媒体",
        extensions: ["mp3", "wav", "m4a", "aac", "flac", "ogg", "opus", "mp4", "m4v", "mov", "webm", "mkv", "avi"],
      }],
    });
    if (!selected || Array.isArray(selected)) return;

    try {
      const media = await backend.importMedia(selected);
      await refreshLibrary();
      await loadMediaById(media.id, true);
      await startAutoAsr(media);
      setActivePage("library");
    } catch (err) {
      console.error(err);
      setStatus(formatErrorMessage(err), "warning");
    }
  });

  // Manual subtitle import
  dom.importSubtitleButton.addEventListener("click", async () => {
    if (!currentMediaId) {
      setStatus("请先选择一个素材", "warning");
      return;
    }

    const selected = await open({
      multiple: false,
      filters: [{ name: "字幕", extensions: ["srt", "vtt"] }],
    });
    if (!selected || Array.isArray(selected)) return;

    try {
      await loadSubtitleFromPath(selected);
      await backend.updateMediaSubtitle(currentMediaId, selected);
      await refreshLibrary();
      dom.retryAsrButton.style.display = "none";
      setStatus("手动字幕已导入并绑定", "success");
    } catch (err) {
      console.error(err);
      setStatus(formatErrorMessage(err), "warning");
    }
  });

  // ASR retry
  dom.retryAsrButton.addEventListener("click", async () => {
    const media = getCurrentMedia();
    if (!media) return;
    await startAutoAsr(media);
  });

  dom.subtitleEditorList.addEventListener("input", (event) => {
    if (!activeSubtitleDocument) return;
    const target = event.target;
    if (!(target instanceof HTMLTextAreaElement)) return;

    const row = target.closest<HTMLElement>("[data-cue-index]");
    const cueIndex = Number(row?.dataset.cueIndex);
    if (!Number.isFinite(cueIndex)) return;

    const cue = activeSubtitleDocument.cues[cueIndex];
    if (!cue) return;

    if (target.dataset.field === "text") {
      cue.text = target.value;
      return;
    }

    if (target.dataset.field === "secondaryText") {
      cue.secondaryText = target.value;
    }
  });

  dom.subtitleEditorBackButton.addEventListener("click", () => {
    setActivePage(lastMainPage);
  });

  dom.subtitleEditorSaveButton.addEventListener("click", () => {
    void saveSubtitleEditor();
  });

  /* ===========================================================
     Event bindings — Player
     =========================================================== */

  dom.playToggleButton.addEventListener("click", () => { void player.togglePlayback(); });
  dom.progressInput.addEventListener("input", () => { player.seek(Number(dom.progressInput.value)); });

  dom.playbackRateSelect.addEventListener("change", async () => {
    settings.playbackRate = Number(dom.playbackRateSelect.value);
    player.setPlaybackRate(settings.playbackRate);
    await persistSettings();
    setStatus(`播放倍率已更新为 ${settings.playbackRate.toFixed(2)}x`, "success");
  });

  dom.playlistModeSelect.addEventListener("change", async () => {
    settings.playlistMode = dom.playlistModeSelect.value as PlaylistMode;
    await persistSettings();
    setStatus(settings.playlistMode === "single" ? "已切换为单曲循环" : "已切换为顺序播放", "success");
  });

  /* ===========================================================
     Event bindings — Settings: Overlay
     =========================================================== */

  dom.overlayVisibleCheckbox.addEventListener("change", async () => {
    settings.overlayVisible = dom.overlayVisibleCheckbox.checked;
    await persistSettings();
    if (settings.overlayVisible) await backend.showOverlay();
    else await backend.hideOverlay();
    await syncOverlay(player.getSnapshot());
  });

  // 锁定/解锁悬浮窗（从主窗口控制）
  const updateLockToggleUi = () => {
    dom.overlayLockToggle.dataset.locked = String(overlayLocked);
    dom.overlayLockToggle.textContent = overlayLocked ? "解锁窗口" : "锁定窗口";
  };

  dom.overlayLockToggle.addEventListener("click", async () => {
    overlayLocked = !overlayLocked;
    updateLockToggleUi();
    await emitTo("overlay", OVERLAY_LOCK_EVENT, { locked: overlayLocked });
  });

  // 当悬浮窗自己锁定时（点击悬浮窗上的锁定按钮），同步状态到主窗口
  const unlistenOverlayLock = await listen<{ locked: boolean }>(OVERLAY_LOCK_EVENT, ({ payload }) => {
    overlayLocked = payload.locked;
    updateLockToggleUi();
  });
  unlisteners.push(unlistenOverlayLock);

  // 当悬浮窗点击关闭按钮时，同步关闭设置中的「开启悬浮字幕窗」
  const unlistenOverlayClose = await listen(OVERLAY_CLOSE_EVENT, async () => {
    settings.overlayVisible = false;
    dom.overlayVisibleCheckbox.checked = false;
    await persistSettings();
  });
  unlisteners.push(unlistenOverlayClose);

  dom.overlayPositionSelect.addEventListener("change", async () => {
    settings.overlay.position = dom.overlayPositionSelect.value as OverlaySettings["position"];
    await persistSettings();
  });

  dom.overlayColorInput.addEventListener("input", async () => {
    settings.overlay.color = dom.overlayColorInput.value;
    await overlayBridge.updateStyle(settings.overlay);
  });
  dom.overlayColorInput.addEventListener("change", () => { void persistSettings(); });

  dom.strokeColorInput.addEventListener("input", async () => {
    settings.overlay.strokeColor = dom.strokeColorInput.value;
    await overlayBridge.updateStyle(settings.overlay);
  });
  dom.strokeColorInput.addEventListener("change", () => { void persistSettings(); });

  dom.secondaryColorInput.addEventListener("input", async () => {
    settings.overlay.secondaryColor = dom.secondaryColorInput.value;
    await overlayBridge.updateStyle(settings.overlay);
  });
  dom.secondaryColorInput.addEventListener("change", () => { void persistSettings(); });

  dom.secondaryStrokeColorInput.addEventListener("input", async () => {
    settings.overlay.secondaryStrokeColor = dom.secondaryStrokeColorInput.value;
    await overlayBridge.updateStyle(settings.overlay);
  });
  dom.secondaryStrokeColorInput.addEventListener("change", () => { void persistSettings(); });

  dom.fontSizeInput.addEventListener("input", async () => {
    settings.overlay.fontSize = Number(dom.fontSizeInput.value);
    applyOverlayPreview(settings.overlay);
    await overlayBridge.updateStyle(settings.overlay);
  });
  dom.fontSizeInput.addEventListener("change", () => { void persistSettings(); });

  dom.opacityInput.addEventListener("input", async () => {
    settings.overlay.opacity = Number(dom.opacityInput.value) / 100;
    applyOverlayPreview(settings.overlay);
    await overlayBridge.updateStyle(settings.overlay);
  });
  dom.opacityInput.addEventListener("change", () => { void persistSettings(); });

  /* ===========================================================
     Event bindings — Settings: Danger zone (all with confirm)
     =========================================================== */

  dom.clearSubtitlesButton.addEventListener("click", async () => {
    if (!await showConfirm("清理字幕缓存", "所有已生成的字幕文件将被删除，素材不受影响。")) return;
    try {
      const r = await backend.clearSubtitles();
      await refreshLibrary();
      setStatus(`字幕缓存已清理，${fmtCleanup(r)}`, "success");
    } catch (err) { console.error(err); setStatus("清理字幕缓存失败", "warning"); }
  });

  dom.clearAudioCacheButton.addEventListener("click", async () => {
    if (!await showConfirm("清理音频缓存", "识别过程的中间音频文件将被删除。")) return;
    try {
      const r = await backend.clearAudioCache();
      setStatus(`音频缓存已清理，${fmtCleanup(r)}`, "success");
    } catch (err) { console.error(err); setStatus("清理音频缓存失败", "warning"); }
  });

  dom.deleteModelButton.addEventListener("click", async () => {
    if (!await showConfirm("删除所有离线模型", "所有已下载的模型将被删除，需要重新下载才能离线识别。")) return;
    try {
      const r = await backend.deleteDefaultModel();
      // Refresh all model statuses
      const allStatus = await backend.getAllModelsStatus();
      modelsStatusMap.clear();
      for (const s of allStatus.models) modelsStatusMap.set(s.modelId, s);
      modelStatus = modelsStatusMap.get(settings.selectedModel);
      updateModelUi();
      setStatus(`所有模型已删除，${fmtCleanup(r)}`, "success");
    } catch (err) { console.error(err); setStatus("删除模型失败", "warning"); }
  });

  dom.resetAppDataButton.addEventListener("click", async () => {
    if (!await showConfirm("重置全部数据", "所有素材、字幕、模型、设置都将被清空！此操作不可逆。")) return;
    try {
      const r = await backend.resetAppData();
      settings = { ...DEFAULT_SETTINGS };
      // Refresh all model statuses after reset
      const allStatus = await backend.getAllModelsStatus();
      modelsStatusMap.clear();
      for (const s of allStatus.models) modelsStatusMap.set(s.modelId, s);
      modelStatus = modelsStatusMap.get(settings.selectedModel);
      applyOverlayPreview(settings.overlay);
      await backend.hideOverlay();
      await resetPlaybackUi();
      await refreshLibrary();
      updateModelUi();
      setStatus(`应用数据已重置，${fmtCleanup(r)}`, "success");
    } catch (err) { console.error(err); setStatus("重置应用数据失败", "warning"); }
  });

  /* ===========================================================
     Player subscriptions
     =========================================================== */

  player.subscribe((snap) => {
    renderTransport(snap);
    renderSubtitle(snap);
    void syncOverlay(snap);
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

  /* ===========================================================
     Keyboard shortcuts
     =========================================================== */

  document.addEventListener("keydown", (e) => {
    // Don't capture shortcuts when user is in an input/select
    if (e.target instanceof HTMLInputElement || e.target instanceof HTMLSelectElement || e.target instanceof HTMLTextAreaElement) return;

    switch (e.code) {
      case "Space":
        e.preventDefault();
        if (player.hasMedia()) void player.togglePlayback();
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
  });

  /* ===========================================================
     ASR event listeners (with proper cleanup refs)
     =========================================================== */

  const unAsrStarted = await asrEvents.onStarted(({ jobId }) => {
    activeAsrJobId = jobId;
    dom.statusBadgeLabel.textContent = "运行中";
  });
  unlisteners.push(unAsrStarted);

  const unAsrProgress = await asrEvents.onProgress(({ jobId, message }) => {
    if (activeAsrJobId && activeAsrJobId !== jobId) return;
    setStatus(message, "neutral");
  });
  unlisteners.push(unAsrProgress);

  const unAsrCompleted = await asrEvents.onCompleted(async ({ jobId, subtitlePath, detectedLanguage }) => {
    if (activeAsrJobId !== jobId) return;
    activeAsrJobId = undefined;

    try {
      let finalSubtitlePath = subtitlePath;
      let translationError: string | undefined;
      const shouldTranslateToChinese = !isChineseLanguage(detectedLanguage);
      if (pendingSubtitleMediaId) {
        await backend.updateMediaSubtitle(pendingSubtitleMediaId, subtitlePath);
        if (shouldTranslateToChinese) {
          try {
            const translated = await backend.translateMediaSubtitle(pendingSubtitleMediaId, detectedLanguage);
            finalSubtitlePath = translated.subtitlePath;
          } catch (err) {
            console.error(err);
            translationError = formatErrorMessage(err);
          }
        }
      }
      await refreshLibrary();
      if (pendingSubtitleMediaId && currentMediaId === pendingSubtitleMediaId) {
        await loadSubtitleFromPath(finalSubtitlePath);
        dom.retryAsrButton.style.display = "none";
      }
      setStatus(
        translationError
          ? `离线识别完成，但中文字幕生成失败：${translationError}`
          : shouldTranslateToChinese
            ? "离线识别完成，双语字幕已绑定"
            : "离线识别完成，原文字幕已绑定",
        translationError ? "warning" : "success",
      );
    } catch (err) {
      console.error(err);
      setStatus("识别完成，但字幕绑定失败", "warning");
    } finally {
      pendingSubtitleMediaId = undefined;
    }
  });
  unlisteners.push(unAsrCompleted);

  const unAsrFailed = await asrEvents.onFailed(({ jobId, code, message }) => {
    if (activeAsrJobId && activeAsrJobId !== jobId) return;
    activeAsrJobId = undefined;
    pendingSubtitleMediaId = undefined;
    dom.retryAsrButton.style.display = "";
    setStatus(`[${code}] ${message}`, "warning");
  });
  unlisteners.push(unAsrFailed);

  /* ===========================================================
     Model download event listeners
     =========================================================== */

  const unModelStarted = await modelEvents.onStarted(({ jobId }) => {
    activeModelDownloadJobId = jobId;
    updateModelUi();
  });
  unlisteners.push(unModelStarted);

  const unModelProgress = await modelEvents.onProgress(({ jobId, message }) => {
    if (activeModelDownloadJobId && activeModelDownloadJobId !== jobId) return;
    setStatus(message, "neutral");
  });
  unlisteners.push(unModelProgress);

  const unModelCompleted = await modelEvents.onCompleted(({ jobId, status }) => {
    if (activeModelDownloadJobId && activeModelDownloadJobId !== jobId) return;
    activeModelDownloadJobId = undefined;
    modelsStatusMap.set(status.modelId, status);
    if (status.modelId === settings.selectedModel) modelStatus = status;
    updateModelUi();
    const label = availableModels.find((m) => m.id === status.modelId)?.label ?? status.modelId;
    setStatus(`模型 ${label} 下载完成`, "success");
  });
  unlisteners.push(unModelCompleted);

  const unModelFailed = await modelEvents.onFailed(({ jobId, code, message }) => {
    if (activeModelDownloadJobId && activeModelDownloadJobId !== jobId) return;
    activeModelDownloadJobId = undefined;
    updateModelUi();
    setStatus(`[${code}] ${message}`, "warning");
  });
  unlisteners.push(unModelFailed);

  /* ===========================================================
     Init
     =========================================================== */

  try {
    settings = await backend.getSettings();
    // Ensure playlistMode has a value (migration from old settings)
    if (!settings.playlistMode) settings.playlistMode = "sequential";
    dom.playbackRateSelect.value = settings.playbackRate.toFixed(2);
    dom.playlistModeSelect.value = settings.playlistMode;
    player.setPlaybackRate(settings.playbackRate);
    applyOverlayPreview(settings.overlay);
    await overlayBridge.updateStyle(settings.overlay);
    if (settings.overlayVisible) await backend.showOverlay();
    setStatus("设置已加载", "success");
  } catch (err) {
    console.error(err);
    setStatus("读取设置失败，已使用默认配置", "warning");
  }

  try {
    await refreshLibrary();
  } catch (err) {
    console.error(err);
    setStatus("读取素材库失败", "warning");
  }

  try {
    availableModels = await backend.getAvailableModels();
    const allStatus = await backend.getAllModelsStatus();
    modelsStatusMap.clear();
    for (const s of allStatus.models) modelsStatusMap.set(s.modelId, s);
    modelStatus = modelsStatusMap.get(settings.selectedModel);
    updateModelUi();
  } catch (err) {
    console.error(err);
    dom.modelStatusLabel.textContent = "模型状态读取失败";
  }

  applyOverlayPreview(settings.overlay);
  renderTransport(player.getSnapshot());
  renderSubtitle(player.getSnapshot());
}
