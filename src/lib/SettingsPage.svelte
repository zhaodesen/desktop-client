<script lang="ts">
  import type { AppSettings, ModelInfo, ModelStatus, OverlaySettings } from "../shared/types";
  import ModelList from "./ModelList.svelte";

  interface Props {
    settings: AppSettings;
    availableModels: ModelInfo[];
    modelsStatusMap: Map<string, ModelStatus>;
    isDownloading: boolean;
    modelStatusLabel: string;
    modelPathLabel: string;
    overlayLocked: boolean;
    onOverlayVisibleChange: (v: boolean) => void;
    onOverlayLockToggle: () => void;
    onOverlayStyleChange: (overlay: OverlaySettings) => void;
    onOverlayStyleCommit: () => void;
    onDownloadModel: (id: string) => void;
    onSelectModel: (id: string) => void;
    onDeleteModel: (id: string) => void;
    onClearSubtitles: () => void;
    onClearAudioCache: () => void;
    onDeleteAllModels: () => void;
    onResetAppData: () => void;
  }

  const {
    settings, availableModels, modelsStatusMap, isDownloading,
    modelStatusLabel, modelPathLabel, overlayLocked,
    onOverlayVisibleChange, onOverlayLockToggle, onOverlayStyleChange, onOverlayStyleCommit,
    onDownloadModel, onSelectModel, onDeleteModel,
    onClearSubtitles, onClearAudioCache, onDeleteAllModels, onResetAppData,
  }: Props = $props();

  const fontSizeDisplay = $derived(`${Math.round(settings.overlay.fontSize)}px`);
  const opacityDisplay = $derived(`${Math.round(settings.overlay.opacity * 100)}%`);
  const opacityPct = $derived(Math.round(settings.overlay.opacity * 100));
</script>

<section class="page" data-active="true">
  <header class="page-header">
    <h2>设置</h2>
  </header>

  <div class="settings-layout">
    <!-- 悬浮窗设置 -->
    <div class="card settings-card">
      <h4>悬浮窗</h4>
      <label class="toggle-row">
        <span>开启悬浮字幕窗</span>
        <input
          type="checkbox"
          class="toggle"
          checked={settings.overlayVisible}
          onchange={(e) => onOverlayVisibleChange((e.target as HTMLInputElement).checked)}
        />
      </label>
      <div class="form-row">
        <span>窗口交互</span>
        <button
          class="btn btn-sm btn-outline"
          type="button"
          data-locked={overlayLocked}
          id="overlay-lock-toggle"
          onclick={onOverlayLockToggle}
        >
          {overlayLocked ? "解锁窗口" : "锁定窗口"}
        </button>
      </div>
      <label class="form-row">
        <span>位置</span>
        <select
          value={settings.overlay.position}
          onchange={(e) => {
            onOverlayStyleChange({ ...settings.overlay, position: (e.target as HTMLSelectElement).value as OverlaySettings["position"] });
            onOverlayStyleCommit();
          }}
        >
          <option value="bottom">底部</option>
          <option value="top">顶部</option>
        </select>
      </label>

      <div class="settings-group-label">原文字幕</div>
      <label class="form-row">
        <span>字体颜色</span>
        <input
          type="color"
          value={settings.overlay.color}
          oninput={(e) => onOverlayStyleChange({ ...settings.overlay, color: (e.target as HTMLInputElement).value })}
          onchange={onOverlayStyleCommit}
        />
      </label>
      <label class="form-row">
        <span>描边颜色</span>
        <input
          type="color"
          value={settings.overlay.strokeColor}
          oninput={(e) => onOverlayStyleChange({ ...settings.overlay, strokeColor: (e.target as HTMLInputElement).value })}
          onchange={onOverlayStyleCommit}
        />
      </label>

      <div class="settings-group-label">中文字幕</div>
      <label class="form-row">
        <span>字体颜色</span>
        <input
          type="color"
          value={settings.overlay.secondaryColor}
          oninput={(e) => onOverlayStyleChange({ ...settings.overlay, secondaryColor: (e.target as HTMLInputElement).value })}
          onchange={onOverlayStyleCommit}
        />
      </label>
      <label class="form-row">
        <span>描边颜色</span>
        <input
          type="color"
          value={settings.overlay.secondaryStrokeColor}
          oninput={(e) => onOverlayStyleChange({ ...settings.overlay, secondaryStrokeColor: (e.target as HTMLInputElement).value })}
          onchange={onOverlayStyleCommit}
        />
      </label>

      <label class="form-row">
        <span>字体大小</span>
        <div class="range-group">
          <input
            type="range"
            min="24"
            max="60"
            step="1"
            value={settings.overlay.fontSize}
            oninput={(e) => onOverlayStyleChange({ ...settings.overlay, fontSize: Number((e.target as HTMLInputElement).value) })}
            onchange={onOverlayStyleCommit}
          />
          <strong>{fontSizeDisplay}</strong>
        </div>
      </label>
      <label class="form-row">
        <span>透明度</span>
        <div class="range-group">
          <input
            type="range"
            min="60"
            max="100"
            step="1"
            value={opacityPct}
            oninput={(e) => onOverlayStyleChange({ ...settings.overlay, opacity: Number((e.target as HTMLInputElement).value) / 100 })}
            onchange={onOverlayStyleCommit}
          />
          <strong>{opacityDisplay}</strong>
        </div>
      </label>
    </div>

    <!-- 离线模型 -->
    <div class="card settings-card">
      <h4>离线模型</h4>
      <ModelList
        {availableModels}
        {modelsStatusMap}
        selectedModel={settings.selectedModel}
        {isDownloading}
        statusLabel={modelStatusLabel}
        pathLabel={modelPathLabel}
        onDownload={onDownloadModel}
        onSelect={onSelectModel}
        onDelete={onDeleteModel}
      />
    </div>

    <!-- 数据管理 -->
    <div class="card settings-card">
      <h4>数据管理</h4>
      <p class="text-dim text-xs">以下操作不可逆，请谨慎执行。</p>
      <div class="danger-grid">
        <button class="btn btn-outline btn-danger" type="button" onclick={onClearSubtitles}>清理字幕缓存</button>
        <button class="btn btn-outline btn-danger" type="button" onclick={onClearAudioCache}>清理音频缓存</button>
        <button class="btn btn-outline btn-danger" type="button" onclick={onDeleteAllModels}>删除所有模型</button>
        <button class="btn btn-danger-solid" type="button" onclick={onResetAppData}>重置全部数据</button>
      </div>
    </div>
  </div>
</section>
