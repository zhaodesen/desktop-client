import { convertFileSrc } from "@tauri-apps/api/core";
import { emitTo, listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { OVERLAY_LOCK_EVENT } from "../shared/events";
import { asrEvents, backend, modelEvents, overlayBridge } from "../shared/tauri";
import type {
  AppSettings,
  CleanupResult,
  DefaultModelStatus,
  LibraryState,
  MediaItem,
  OverlaySettings,
  PlaybackSnapshot,
  PlaylistMode,
  SubtitleContext,
} from "../shared/types";
import { PlayerController } from "./player-controller";
import { parseSubtitleText } from "./subtitle-parser";
import { SubtitleEngine } from "./subtitle-engine";

/* ============================================================
   Helpers
   ============================================================ */

const DEFAULT_SETTINGS: AppSettings = {
  playbackRate: 1,
  overlayVisible: false,
  overlay: { fontSize: 34, opacity: 1.0, color: "#ffffff", position: "bottom" },
  playlistMode: "sequential",
};

function queryElement<T extends Element>(selector: string): T {
  const element = document.querySelector<T>(selector);
  if (!element) throw new Error(`找不到元素: ${selector}`);
  return element;
}

function formatDuration(ms: number): string {
  const t = Math.max(0, Math.floor(ms / 1000));
  const h = Math.floor(t / 3600);
  const m = Math.floor((t % 3600) / 60);
  const s = t % 60;
  const mm = String(m).padStart(2, "0");
  const ss = String(s).padStart(2, "0");
  return h > 0 ? `${String(h).padStart(2, "0")}:${mm}:${ss}` : `${mm}:${ss}`;
}

function formatTimestamp(ts: number): string {
  return new Date(ts).toLocaleString("zh-CN", { month: "2-digit", day: "2-digit", hour: "2-digit", minute: "2-digit" });
}

function escapeHtml(v: string): string {
  return v.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;").replace(/'/g, "&#39;");
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
  playbackRateSelect: HTMLSelectElement;
  playlistModeSelect: HTMLSelectElement;
  retryAsrButton: HTMLButtonElement;
  overlayVisibleCheckbox: HTMLInputElement;
  overlayPositionSelect: HTMLSelectElement;
  overlayColorInput: HTMLInputElement;
  fontSizeInput: HTMLInputElement;
  opacityInput: HTMLInputElement;
  fontSizeValue: HTMLElement;
  opacityValue: HTMLElement;
  downloadModelButton: HTMLButtonElement;
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
    playbackRateSelect: queryElement("#playback-rate-select"),
    playlistModeSelect: queryElement("#playlist-mode-select"),
    retryAsrButton: queryElement("#retry-asr-button"),
    overlayVisibleCheckbox: queryElement("#overlay-visible-checkbox"),
    overlayPositionSelect: queryElement("#overlay-position-select"),
    overlayColorInput: queryElement("#overlay-color-input"),
    fontSizeInput: queryElement("#font-size-input"),
    opacityInput: queryElement("#opacity-input"),
    fontSizeValue: queryElement("#font-size-value"),
    opacityValue: queryElement("#opacity-value"),
    downloadModelButton: queryElement("#download-model-button"),
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

  // ---- Getters ----
  const getCurrentMedia = (): MediaItem | undefined =>
    libraryState.mediaItems.find((item) => item.id === currentMediaId);

  const getSubtitleContext = (snap: PlaybackSnapshot): SubtitleContext =>
    subtitleEngine.getContext(snap.currentTimeMs);

  const fmtCleanup = (r: CleanupResult) =>
    `已删除 ${r.deletedFiles} 个文件，${r.deletedDirs} 个目录`;

  // ---- Overlay ----
  const applyOverlayPreview = (ov: OverlaySettings) => {
    dom.overlayVisibleCheckbox.checked = settings.overlayVisible;
    dom.overlayPositionSelect.value = ov.position;
    dom.overlayColorInput.value = ov.color;
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
    if (!modelStatus) {
      dom.modelStatusLabel.textContent = "正在检查模型状态…";
      dom.modelPathLabel.textContent = "推荐路径：appData/models/ggml-base.bin";
      dom.downloadModelButton.disabled = Boolean(activeModelDownloadJobId);
      return;
    }
    dom.modelStatusLabel.textContent = modelStatus.installed
      ? `已就绪 · 来源 ${modelStatus.source}`
      : "未安装默认模型";
    dom.modelPathLabel.textContent = modelStatus.path ?? "模型会下载到 appData/models/ggml-base.bin";
    dom.downloadModelButton.disabled = modelStatus.installed || Boolean(activeModelDownloadJobId);
  };

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
      setStatus(err instanceof Error ? err.message : "自动生成字幕失败", "warning");
    }
  };

  /* ===========================================================
     Event bindings — Tabs
     =========================================================== */

  dom.tabTriggers.forEach((trigger) => {
    trigger.addEventListener("click", () => {
      const tab = trigger.dataset.tabTrigger;
      dom.tabTriggers.forEach((t) => { t.dataset.active = String(t === trigger); });
      dom.tabPanels.forEach((p) => { p.dataset.active = String(p.dataset.tabPanel === tab); });
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
      dom.tabTriggers[0]?.click();
    } catch (err) {
      console.error(err);
      setStatus(err instanceof Error ? err.message : "导入媒体失败", "warning");
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
      setStatus(err instanceof Error ? err.message : "导入字幕失败", "warning");
    }
  });

  // ASR retry
  dom.retryAsrButton.addEventListener("click", async () => {
    const media = getCurrentMedia();
    if (!media) return;
    await startAutoAsr(media);
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

  dom.overlayPositionSelect.addEventListener("change", async () => {
    settings.overlay.position = dom.overlayPositionSelect.value as OverlaySettings["position"];
    await persistSettings();
  });

  dom.overlayColorInput.addEventListener("input", async () => {
    settings.overlay.color = dom.overlayColorInput.value;
    await overlayBridge.updateStyle(settings.overlay);
  });
  dom.overlayColorInput.addEventListener("change", () => { void persistSettings(); });

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
     Event bindings — Settings: Model
     =========================================================== */

  dom.downloadModelButton.addEventListener("click", async () => {
    try {
      const { jobId } = await backend.downloadDefaultModel();
      activeModelDownloadJobId = jobId;
      updateModelUi();
      setStatus("默认模型开始下载", "neutral");
    } catch (err) {
      console.error(err);
      setStatus("启动模型下载失败", "warning");
    }
  });

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
    if (!await showConfirm("删除离线模型", "删除后需要重新下载才能离线识别。")) return;
    try {
      const r = await backend.deleteDefaultModel();
      modelStatus = await backend.getDefaultModelStatus();
      updateModelUi();
      setStatus(`默认模型已删除，${fmtCleanup(r)}`, "success");
    } catch (err) { console.error(err); setStatus("删除默认模型失败", "warning"); }
  });

  dom.resetAppDataButton.addEventListener("click", async () => {
    if (!await showConfirm("重置全部数据", "所有素材、字幕、模型、设置都将被清空！此操作不可逆。")) return;
    try {
      const r = await backend.resetAppData();
      settings = { ...DEFAULT_SETTINGS };
      modelStatus = await backend.getDefaultModelStatus();
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

  const unAsrCompleted = await asrEvents.onCompleted(async ({ jobId, subtitlePath }) => {
    if (activeAsrJobId !== jobId) return;
    activeAsrJobId = undefined;

    try {
      if (pendingSubtitleMediaId) {
        await backend.updateMediaSubtitle(pendingSubtitleMediaId, subtitlePath);
      }
      await refreshLibrary();
      if (pendingSubtitleMediaId && currentMediaId === pendingSubtitleMediaId) {
        await loadSubtitleFromPath(subtitlePath);
        dom.retryAsrButton.style.display = "none";
      }
      setStatus("离线识别完成，字幕已绑定", "success");
    } catch (err) {
      console.error(err);
      setStatus("识别完成，但字幕绑定失败", "warning");
    } finally {
      pendingSubtitleMediaId = undefined;
    }
  });
  unlisteners.push(unAsrCompleted);

  const unAsrFailed = await asrEvents.onFailed(({ jobId, message }) => {
    if (activeAsrJobId && activeAsrJobId !== jobId) return;
    activeAsrJobId = undefined;
    pendingSubtitleMediaId = undefined;
    dom.retryAsrButton.style.display = "";
    setStatus(message, "warning");
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
    modelStatus = status;
    updateModelUi();
    setStatus("默认模型下载完成", "success");
  });
  unlisteners.push(unModelCompleted);

  const unModelFailed = await modelEvents.onFailed(({ jobId, message }) => {
    if (activeModelDownloadJobId && activeModelDownloadJobId !== jobId) return;
    activeModelDownloadJobId = undefined;
    updateModelUi();
    setStatus(message, "warning");
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
    modelStatus = await backend.getDefaultModelStatus();
    updateModelUi();
  } catch (err) {
    console.error(err);
    dom.modelStatusLabel.textContent = "模型状态读取失败";
  }

  applyOverlayPreview(settings.overlay);
  renderTransport(player.getSnapshot());
  renderSubtitle(player.getSnapshot());
}
