import { convertFileSrc } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { asrEvents, backend, modelEvents, overlayBridge } from "../shared/tauri";
import type {
  AppSettings,
  CleanupResult,
  DefaultModelStatus,
  LibraryState,
  MediaItem,
  OverlaySettings,
  PlaybackSnapshot,
  SubtitleContext,
} from "../shared/types";
import { PlayerController } from "./player-controller";
import { parseSubtitleText } from "./subtitle-parser";
import { SubtitleEngine } from "./subtitle-engine";

const DEFAULT_SETTINGS: AppSettings = {
  playbackRate: 1,
  overlayVisible: false,
  overlay: {
    fontSize: 34,
    opacity: 0.92,
    color: "#fff4d6",
    position: "bottom",
  },
};

type StatusTone = "neutral" | "success" | "warning";
type PlaylistMode = "single" | "sequential";

type DomRefs = {
  audioElement: HTMLAudioElement;
  importMediaButton: HTMLButtonElement;
  tabTriggers: NodeListOf<HTMLButtonElement>;
  tabPanels: NodeListOf<HTMLElement>;
  statusBadge: HTMLElement;
  statusText: HTMLElement;
  libraryCountLabel: HTMLElement;
  mediaLibraryList: HTMLElement;
  playlistCountLabel: HTMLElement;
  playbackHistoryList: HTMLElement;
  audioFileLabel: HTMLElement;
  subtitleFileLabel: HTMLElement;
  cueTiming: HTMLElement;
  playToggleButton: HTMLButtonElement;
  progressInput: HTMLInputElement;
  transportTime: HTMLElement;
  durationMeta: HTMLElement;
  currentText: HTMLElement;
  playbackRateSelect: HTMLSelectElement;
  playlistModeSelect: HTMLSelectElement;
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
};

function queryElement<T extends Element>(selector: string): T {
  const element = document.querySelector<T>(selector);
  if (!element) {
    throw new Error(`找不到元素: ${selector}`);
  }
  return element;
}

function getDomRefs(): DomRefs {
  return {
    audioElement: queryElement("#audio-element"),
    importMediaButton: queryElement("#import-audio-button"),
    tabTriggers: document.querySelectorAll<HTMLButtonElement>("[data-tab-trigger]"),
    tabPanels: document.querySelectorAll<HTMLElement>("[data-tab-panel]"),
    statusBadge: queryElement("#status-badge"),
    statusText: queryElement("#status-text"),
    libraryCountLabel: queryElement("#library-count-label"),
    mediaLibraryList: queryElement("#media-library-list"),
    playlistCountLabel: queryElement("#playlist-count-label"),
    playbackHistoryList: queryElement("#playback-history-list"),
    audioFileLabel: queryElement("#audio-file-label"),
    subtitleFileLabel: queryElement("#subtitle-file-label"),
    cueTiming: queryElement("#cue-timing"),
    playToggleButton: queryElement("#play-toggle-button"),
    progressInput: queryElement("#progress-input"),
    transportTime: queryElement("#transport-time"),
    durationMeta: queryElement("#duration-meta"),
    currentText: queryElement("#current-text"),
    playbackRateSelect: queryElement("#playback-rate-select"),
    playlistModeSelect: queryElement("#playlist-mode-select"),
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
  };
}

function formatDuration(ms: number): string {
  const totalSeconds = Math.max(0, Math.floor(ms / 1000));
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;

  if (hours > 0) {
    return `${String(hours).padStart(2, "0")}:${String(minutes).padStart(2, "0")}:${String(seconds).padStart(2, "0")}`;
  }

  return `${String(minutes).padStart(2, "0")}:${String(seconds).padStart(2, "0")}`;
}

function formatTimestamp(timestamp: number): string {
  return new Date(timestamp).toLocaleString("zh-CN", {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  });
}

function escapeHtml(value: string): string {
  return value
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

export async function bootstrapMainApp(): Promise<void> {
  const dom = getDomRefs();
  const subtitleEngine = new SubtitleEngine();
  const player = new PlayerController(dom.audioElement);

  let settings = DEFAULT_SETTINGS;
  let libraryState: LibraryState = { mediaItems: [], playbackHistory: [] };
  let currentMediaId: string | undefined;
  let activeAsrJobId: string | undefined;
  let activeModelDownloadJobId: string | undefined;
  let pendingSubtitleMediaId: string | undefined;
  let modelStatus: DefaultModelStatus | undefined;
  let playlistMode: PlaylistMode = "sequential";

  const setStatus = (text: string, tone: StatusTone = "neutral") => {
    dom.statusText.textContent = text;
    dom.statusBadge.dataset.tone = tone;
    dom.statusBadge.textContent = activeAsrJobId || activeModelDownloadJobId
      ? "运行中"
      : tone === "success"
        ? "完成"
        : tone === "warning"
          ? "注意"
          : "就绪";
  };

  const getCurrentMedia = (): MediaItem | undefined =>
    libraryState.mediaItems.find((item) => item.id === currentMediaId);

  const getCurrentSubtitleContext = (
    snapshot: PlaybackSnapshot,
  ): SubtitleContext => subtitleEngine.getContext(snapshot.currentTimeMs);

  const formatCleanupSummary = (result: CleanupResult) =>
    `已删除 ${result.deletedFiles} 个文件，${result.deletedDirs} 个目录`;

  const applyOverlayControlPreview = (overlay: OverlaySettings) => {
    dom.overlayVisibleCheckbox.checked = settings.overlayVisible;
    dom.overlayPositionSelect.value = overlay.position;
    dom.overlayColorInput.value = overlay.color;
    dom.fontSizeInput.value = String(overlay.fontSize);
    dom.opacityInput.value = String(Math.round(overlay.opacity * 100));
    dom.fontSizeValue.textContent = `${Math.round(overlay.fontSize)} px`;
    dom.opacityValue.textContent = `${Math.round(overlay.opacity * 100)}%`;
  };

  const updateModelUi = () => {
    if (!modelStatus) {
      dom.modelStatusLabel.textContent = "正在检查模型状态";
      dom.modelPathLabel.textContent = "推荐路径：./models/ggml-base.bin";
      dom.downloadModelButton.disabled = Boolean(activeModelDownloadJobId);
      return;
    }

    dom.modelStatusLabel.textContent = modelStatus.installed
      ? `已就绪 · 来源 ${modelStatus.source}`
      : "未安装默认模型";
    dom.modelPathLabel.textContent = modelStatus.path
      ? modelStatus.path
      : "模型会下载到 appData/models/ggml-base.bin";
    dom.downloadModelButton.disabled =
      modelStatus.installed || Boolean(activeModelDownloadJobId);
  };

  const renderTransport = (snapshot: PlaybackSnapshot) => {
    const durationMs = Math.max(snapshot.durationMs, 0);
    dom.playToggleButton.disabled = !player.hasMedia();
    dom.progressInput.disabled = !player.hasMedia();
    dom.progressInput.max = String(Math.max(durationMs, 1));
    dom.progressInput.value = String(
      Math.min(snapshot.currentTimeMs, durationMs || snapshot.currentTimeMs),
    );
    dom.playToggleButton.textContent = snapshot.playing ? "暂停" : "播放";
    dom.transportTime.textContent = `${formatDuration(snapshot.currentTimeMs)} / ${formatDuration(durationMs)}`;
    dom.durationMeta.textContent = player.hasMedia()
      ? `时长 ${formatDuration(durationMs)} · 倍率 ${snapshot.rate.toFixed(2)}x`
      : "等待导入媒体";
  };

  const syncOverlay = async (snapshot: PlaybackSnapshot) => {
    const media = getCurrentMedia();
    const context = getCurrentSubtitleContext(snapshot);
    await overlayBridge.render({
      fileLabel: media?.title,
      previous: undefined,
      current: context.current,
      next: undefined,
      playback: snapshot,
    });
  };

  const renderCurrentSubtitle = (snapshot: PlaybackSnapshot) => {
    const { current } = getCurrentSubtitleContext(snapshot);
    dom.currentText.textContent = current?.text ?? "当前时间点暂无字幕";
    dom.cueTiming.textContent = current
      ? `${formatDuration(current.startMs)} - ${formatDuration(current.endMs)}`
      : "未命中字幕句";
  };

  const renderLibrary = () => {
    dom.libraryCountLabel.textContent = `${libraryState.mediaItems.length} 条`;
    if (libraryState.mediaItems.length === 0) {
      dom.mediaLibraryList.className = "list-stack empty-state";
      dom.mediaLibraryList.textContent = "还没有导入任何素材";
      return;
    }

    dom.mediaLibraryList.className = "list-stack";
    dom.mediaLibraryList.innerHTML = libraryState.mediaItems
      .map((item) => {
        const subtitleState = item.subtitlePath ? "已生成字幕" : "待生成字幕";
        const sourceLabel = item.sourceKind === "video" ? "视频转音频" : "音频素材";
        return `
          <article class="list-item">
            <div class="list-item-head">
              <div>
                <p class="list-item-title">${escapeHtml(item.title)}</p>
                <div class="chip-row">
                  <span class="chip">${sourceLabel}</span>
                  <span class="chip">${subtitleState}</span>
                  <span class="chip">导入于 ${formatTimestamp(item.importedAt)}</span>
                </div>
              </div>
            </div>
            <div class="list-item-foot">
              <span class="muted-text">${escapeHtml(item.originalFileName)}</span>
              <div class="chip-row">
                <button class="button button-ghost" type="button" data-action="play-media" data-media-id="${item.id}">
                  播放
                </button>
                <button class="button" type="button" data-action="delete-media" data-media-id="${item.id}">
                  删除
                </button>
              </div>
            </div>
          </article>
        `;
      })
      .join("");
  };

  const renderPlaylist = () => {
    dom.playlistCountLabel.textContent = `${libraryState.playbackHistory.length} 条`;
    if (libraryState.playbackHistory.length === 0) {
      dom.playbackHistoryList.className = "list-stack empty-state";
      dom.playbackHistoryList.textContent = "还没有播放记录";
      return;
    }

    dom.playbackHistoryList.className = "list-stack";
    dom.playbackHistoryList.innerHTML = libraryState.playbackHistory
      .map((entry) => `
        <article class="list-item">
          <div class="list-item-head">
            <div>
              <p class="list-item-title">${escapeHtml(entry.title)}</p>
              <div class="chip-row">
                <span class="chip">播放 ${entry.playCount} 次</span>
                <span class="chip">最近播放 ${formatTimestamp(entry.playedAt)}</span>
                <span class="chip">${entry.subtitlePath ? "已带字幕" : "无字幕"}</span>
              </div>
            </div>
            <button class="button button-ghost" type="button" data-action="play-history" data-media-id="${entry.mediaId}">
              播放
            </button>
          </div>
        </article>
      `)
      .join("");
  };

  const refreshLibraryState = async () => {
    libraryState = await backend.getLibraryState();
    renderLibrary();
    renderPlaylist();
  };

  const attachListEvents = () => {
    dom.mediaLibraryList.querySelectorAll<HTMLElement>("[data-action='play-media']").forEach((button) => {
      button.addEventListener("click", () => {
        const mediaId = button.dataset.mediaId;
        if (!mediaId) {
          return;
        }
        void loadMediaById(mediaId, true);
      });
    });

    dom.mediaLibraryList.querySelectorAll<HTMLElement>("[data-action='delete-media']").forEach((button) => {
      button.addEventListener("click", () => {
        const mediaId = button.dataset.mediaId;
        if (!mediaId) {
          return;
        }
        void deleteMediaById(mediaId);
      });
    });

    dom.playbackHistoryList.querySelectorAll<HTMLElement>("[data-action='play-history']").forEach((button) => {
      button.addEventListener("click", () => {
        const mediaId = button.dataset.mediaId;
        if (!mediaId) {
          return;
        }
        void loadMediaById(mediaId, true);
      });
    });
  };

  const resetPlaybackUi = async () => {
    subtitleEngine.clear();
    currentMediaId = undefined;
    dom.audioElement.pause();
    dom.audioElement.removeAttribute("src");
    dom.audioElement.load();
    dom.audioFileLabel.textContent = "未选择素材";
    dom.subtitleFileLabel.textContent = "未生成字幕";
    dom.currentText.textContent = "等待播放";
    dom.cueTiming.textContent = "未命中字幕句";
    renderTransport(player.getSnapshot());
    await overlayBridge.clear();
  };

  const loadSubtitleFromPath = async (path: string) => {
    const content = await fetch(convertFileSrc(path)).then((response) => response.text());
    const cues = parseSubtitleText(content);
    if (cues.length === 0) {
      throw new Error("未解析出有效字幕");
    }
    subtitleEngine.load(cues);
    dom.subtitleFileLabel.textContent = `${path.split(/[\\/]/).pop() ?? "字幕"} · ${cues.length} 句`;
  };

  const persistSettings = async () => {
    settings = await backend.updateSettings(settings);
    applyOverlayControlPreview(settings.overlay);
    await overlayBridge.updateStyle(settings.overlay);
  };

  const loadMediaById = async (mediaId: string, recordPlayback: boolean) => {
    const media = libraryState.mediaItems.find((item) => item.id === mediaId);
    if (!media) {
      setStatus("未找到对应素材", "warning");
      return;
    }

    await player.loadUrl(convertFileSrc(media.audioPath));
    player.setPlaybackRate(settings.playbackRate);
    currentMediaId = media.id;
    dom.audioFileLabel.textContent = media.title;
    subtitleEngine.clear();
    dom.subtitleFileLabel.textContent = media.subtitlePath ? "正在加载字幕" : "未生成字幕";

    if (media.subtitlePath) {
      try {
        await loadSubtitleFromPath(media.subtitlePath);
      } catch (error) {
        console.error(error);
        dom.subtitleFileLabel.textContent = "字幕加载失败";
      }
    } else {
      dom.subtitleFileLabel.textContent = "未生成字幕";
    }

    renderCurrentSubtitle(player.getSnapshot());
    renderTransport(player.getSnapshot());
    await syncOverlay(player.getSnapshot());

    if (recordPlayback) {
      await backend.recordPlayback(media.id);
      await refreshLibraryState();
      attachListEvents();
    }
  };

  const deleteMediaById = async (mediaId: string) => {
    await backend.deleteMedia(mediaId);
    if (currentMediaId === mediaId) {
      await resetPlaybackUi();
    }
    await refreshLibraryState();
    attachListEvents();
    setStatus("素材已删除", "success");
  };

  const startAutoAsr = async (media: MediaItem) => {
    try {
      const { jobId } = await backend.startAsrJob({ audioPath: media.audioPath });
      activeAsrJobId = jobId;
      pendingSubtitleMediaId = media.id;
      setStatus("素材已导入，正在离线生成字幕", "neutral");
    } catch (error) {
      console.error(error);
      setStatus(
        error instanceof Error ? error.message : "自动生成字幕失败",
        "warning",
      );
    }
  };

  dom.tabTriggers.forEach((trigger) => {
    trigger.addEventListener("click", () => {
      const tab = trigger.dataset.tabTrigger;
      dom.tabTriggers.forEach((item) => {
        item.dataset.active = String(item === trigger);
      });
      dom.tabPanels.forEach((panel) => {
        panel.dataset.active = String(panel.dataset.tabPanel === tab);
      });
    });
  });

  dom.importMediaButton.addEventListener("click", async () => {
    const selected = await open({
      multiple: false,
      filters: [
        {
          name: "媒体",
          extensions: [
            "mp3",
            "wav",
            "m4a",
            "aac",
            "flac",
            "ogg",
            "opus",
            "mp4",
            "m4v",
            "mov",
            "webm",
            "mkv",
            "avi",
          ],
        },
      ],
    });

    if (!selected || Array.isArray(selected)) {
      return;
    }

    try {
      const media = await backend.importMedia(selected);
      await refreshLibraryState();
      attachListEvents();
      await loadMediaById(media.id, true);
      await startAutoAsr(media);
      dom.tabTriggers[0]?.click();
    } catch (error) {
      console.error(error);
      setStatus(
        error instanceof Error ? error.message : "导入媒体失败",
        "warning",
      );
    }
  });

  dom.playToggleButton.addEventListener("click", () => {
    void player.togglePlayback();
  });

  dom.progressInput.addEventListener("input", () => {
    player.seek(Number(dom.progressInput.value));
  });

  dom.playbackRateSelect.addEventListener("change", async () => {
    settings.playbackRate = Number(dom.playbackRateSelect.value);
    player.setPlaybackRate(settings.playbackRate);
    await persistSettings();
    setStatus(`播放倍率已更新为 ${settings.playbackRate.toFixed(2)}x`, "success");
  });

  dom.playlistModeSelect.addEventListener("change", () => {
    playlistMode = dom.playlistModeSelect.value as PlaylistMode;
    setStatus(
      playlistMode === "single" ? "已切换为单曲循环" : "已切换为顺序循环",
      "success",
    );
  });

  dom.overlayVisibleCheckbox.addEventListener("change", async () => {
    settings.overlayVisible = dom.overlayVisibleCheckbox.checked;
    await persistSettings();
    if (settings.overlayVisible) {
      await backend.showOverlay();
    } else {
      await backend.hideOverlay();
    }
    await syncOverlay(player.getSnapshot());
  });

  dom.overlayPositionSelect.addEventListener("change", async () => {
    settings.overlay.position = dom.overlayPositionSelect.value as OverlaySettings["position"];
    await persistSettings();
  });

  dom.overlayColorInput.addEventListener("input", async () => {
    settings.overlay.color = dom.overlayColorInput.value;
    await overlayBridge.updateStyle(settings.overlay);
  });

  dom.overlayColorInput.addEventListener("change", () => {
    void persistSettings();
  });

  dom.fontSizeInput.addEventListener("input", async () => {
    settings.overlay.fontSize = Number(dom.fontSizeInput.value);
    applyOverlayControlPreview(settings.overlay);
    await overlayBridge.updateStyle(settings.overlay);
  });

  dom.fontSizeInput.addEventListener("change", () => {
    void persistSettings();
  });

  dom.opacityInput.addEventListener("input", async () => {
    settings.overlay.opacity = Number(dom.opacityInput.value) / 100;
    applyOverlayControlPreview(settings.overlay);
    await overlayBridge.updateStyle(settings.overlay);
  });

  dom.opacityInput.addEventListener("change", () => {
    void persistSettings();
  });

  dom.downloadModelButton.addEventListener("click", async () => {
    try {
      const { jobId } = await backend.downloadDefaultModel();
      activeModelDownloadJobId = jobId;
      updateModelUi();
      setStatus("默认模型开始下载", "neutral");
    } catch (error) {
      console.error(error);
      setStatus("启动模型下载失败", "warning");
    }
  });

  dom.clearSubtitlesButton.addEventListener("click", async () => {
    try {
      const result = await backend.clearSubtitles();
      await refreshLibraryState();
      attachListEvents();
      setStatus(`字幕缓存清理完成，${formatCleanupSummary(result)}`, "success");
    } catch (error) {
      console.error(error);
      setStatus("清理字幕缓存失败", "warning");
    }
  });

  dom.clearAudioCacheButton.addEventListener("click", async () => {
    try {
      const result = await backend.clearAudioCache();
      setStatus(`音频缓存清理完成，${formatCleanupSummary(result)}`, "success");
    } catch (error) {
      console.error(error);
      setStatus("清理音频缓存失败", "warning");
    }
  });

  dom.deleteModelButton.addEventListener("click", async () => {
    try {
      const result = await backend.deleteDefaultModel();
      modelStatus = await backend.getDefaultModelStatus();
      updateModelUi();
      setStatus(`默认模型已删除，${formatCleanupSummary(result)}`, "success");
    } catch (error) {
      console.error(error);
      setStatus("删除默认模型失败", "warning");
    }
  });

  dom.resetAppDataButton.addEventListener("click", async () => {
    try {
      const result = await backend.resetAppData();
      settings = DEFAULT_SETTINGS;
      modelStatus = await backend.getDefaultModelStatus();
      applyOverlayControlPreview(settings.overlay);
      await backend.hideOverlay();
      await resetPlaybackUi();
      await refreshLibraryState();
      attachListEvents();
      updateModelUi();
      setStatus(`应用数据已重置，${formatCleanupSummary(result)}`, "success");
    } catch (error) {
      console.error(error);
      setStatus("重置应用数据失败", "warning");
    }
  });

  player.subscribe((snapshot) => {
    renderTransport(snapshot);
    renderCurrentSubtitle(snapshot);
    void syncOverlay(snapshot);
  });

  player.onEnded(() => {
    if (playlistMode === "single") {
      player.seek(0);
      void player.togglePlayback();
      return;
    }

    if (!currentMediaId || libraryState.playbackHistory.length < 2) {
      return;
    }

    const currentIndex = libraryState.playbackHistory.findIndex((item) => item.mediaId === currentMediaId);
    if (currentIndex === -1) {
      return;
    }
    const nextEntry = libraryState.playbackHistory[(currentIndex + 1) % libraryState.playbackHistory.length];
    void loadMediaById(nextEntry.mediaId, true).then(() => player.togglePlayback());
  });

  try {
    settings = await backend.getSettings();
    dom.playbackRateSelect.value = settings.playbackRate.toFixed(2);
    player.setPlaybackRate(settings.playbackRate);
    applyOverlayControlPreview(settings.overlay);
    await overlayBridge.updateStyle(settings.overlay);
    if (settings.overlayVisible) {
      await backend.showOverlay();
    }
    setStatus("已加载本地设置", "success");
  } catch (error) {
    console.error(error);
    setStatus("读取本地设置失败，已使用默认配置", "warning");
  }

  try {
    await refreshLibraryState();
    attachListEvents();
  } catch (error) {
    console.error(error);
    setStatus("读取素材库失败", "warning");
  }

  try {
    modelStatus = await backend.getDefaultModelStatus();
    updateModelUi();
  } catch (error) {
    console.error(error);
    dom.modelStatusLabel.textContent = "模型状态读取失败";
  }

  void asrEvents.onStarted(({ jobId }) => {
    activeAsrJobId = jobId;
    dom.statusBadge.textContent = "运行中";
  });

  void asrEvents.onProgress(({ jobId, message }) => {
    if (activeAsrJobId && activeAsrJobId !== jobId) {
      return;
    }
    setStatus(message, "neutral");
  });

  void asrEvents.onCompleted(async ({ jobId, subtitlePath }) => {
    if (activeAsrJobId !== jobId) {
      return;
    }

    activeAsrJobId = undefined;

    try {
      if (pendingSubtitleMediaId) {
        await backend.updateMediaSubtitle(pendingSubtitleMediaId, subtitlePath);
      }
      await refreshLibraryState();
      attachListEvents();
      if (pendingSubtitleMediaId && currentMediaId === pendingSubtitleMediaId) {
        await loadSubtitleFromPath(subtitlePath);
      }
      setStatus("离线识别完成，字幕已绑定到素材", "success");
    } catch (error) {
      console.error(error);
      setStatus("识别完成，但字幕绑定失败", "warning");
    } finally {
      pendingSubtitleMediaId = undefined;
    }
  });

  void asrEvents.onFailed(({ jobId, message }) => {
    if (activeAsrJobId && activeAsrJobId !== jobId) {
      return;
    }

    activeAsrJobId = undefined;
    pendingSubtitleMediaId = undefined;
    setStatus(message, "warning");
  });

  void modelEvents.onStarted(({ jobId }) => {
    activeModelDownloadJobId = jobId;
    updateModelUi();
  });

  void modelEvents.onProgress(({ jobId, message }) => {
    if (activeModelDownloadJobId && activeModelDownloadJobId !== jobId) {
      return;
    }
    setStatus(message, "neutral");
  });

  void modelEvents.onCompleted(({ jobId, status }) => {
    if (activeModelDownloadJobId && activeModelDownloadJobId !== jobId) {
      return;
    }
    activeModelDownloadJobId = undefined;
    modelStatus = status;
    updateModelUi();
    setStatus("默认模型下载完成", "success");
  });

  void modelEvents.onFailed(({ jobId, message }) => {
    if (activeModelDownloadJobId && activeModelDownloadJobId !== jobId) {
      return;
    }
    activeModelDownloadJobId = undefined;
    updateModelUi();
    setStatus(message, "warning");
  });

  applyOverlayControlPreview(settings.overlay);
  renderTransport(player.getSnapshot());
  renderCurrentSubtitle(player.getSnapshot());
}
