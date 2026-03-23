<script lang="ts">
  import type { AppSettings, ModelInfo, ModelStatus, OverlaySettings, ShortcutSettings } from "../shared/types";
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
    onShortcutChange: (shortcuts: ShortcutSettings) => void;
    onShortcutCommit: () => void;
    onClearAllCache: () => void;
    onDeleteAllModels: () => void;
    onResetAppData: () => void;
  }

  const {
    settings, availableModels, modelsStatusMap, isDownloading,
    modelStatusLabel, modelPathLabel, overlayLocked,
    onOverlayVisibleChange, onOverlayLockToggle, onOverlayStyleChange, onOverlayStyleCommit,
    onDownloadModel, onSelectModel, onDeleteModel,
    onShortcutChange, onShortcutCommit,
    onClearAllCache, onDeleteAllModels, onResetAppData,
  }: Props = $props();

  type TabId = "overlay" | "shortcuts" | "models" | "data";
  type ShortcutField = keyof ShortcutSettings;

  const tabs: { id: TabId; label: string }[] = [
    { id: "overlay", label: "悬浮窗" },
    { id: "shortcuts", label: "快捷键" },
    { id: "models", label: "离线模型" },
    { id: "data",   label: "数据管理" },
  ];

  const shortcutItems: { field: ShortcutField; label: string }[] = [
    { field: "playPause", label: "播放 / 暂停" },
    { field: "previousTrack", label: "播放上一个" },
    { field: "nextTrack", label: "播放下一个" },
    { field: "toggleOverlay", label: "显示 / 隐藏悬浮窗" },
    { field: "volumeUp", label: "音量增加" },
    { field: "volumeDown", label: "音量减小" },
    { field: "showTranslation", label: "显示中文字幕" },
    { field: "showOriginal", label: "显示原文字幕" },
    { field: "showBilingual", label: "显示双字幕" },
  ];

  let activeTab = $state<TabId>("overlay");
  let recordingField = $state<ShortcutField | null>(null);

  const fontSizeDisplay = $derived(`${Math.round(settings.overlay.fontSize)}px`);
  const opacityDisplay = $derived(`${Math.round(settings.overlay.opacity * 100)}%`);
  const opacityPct = $derived(Math.round(settings.overlay.opacity * 100));

  function formatShortcut(code: string): string {
    const labelMap: Record<string, string> = {
      Space: "空格",
      Comma: ",",
      Period: ".",
      Equal: "=",
      Minus: "-",
      Escape: "Esc",
      ArrowUp: "↑",
      ArrowDown: "↓",
      ArrowLeft: "←",
      ArrowRight: "→",
    };
    if (labelMap[code]) return labelMap[code];
    if (code.startsWith("Key")) return code.slice(3).toUpperCase();
    if (code.startsWith("Digit")) return code.slice(5);
    return code;
  }

  async function handleShortcutCapture(field: ShortcutField, event: KeyboardEvent) {
    event.preventDefault();
    event.stopPropagation();
    if (event.code === "Escape") {
      recordingField = null;
      return;
    }
    onShortcutChange({ ...settings.shortcuts, [field]: event.code });
    recordingField = null;
    await onShortcutCommit();
  }
</script>

<section class="page settings-page" data-active="true">
  <!-- Tab bar -->
  <div class="stab-bar">
    {#each tabs as tab}
      <button
        class="stab-btn"
        class:stab-active={activeTab === tab.id}
        type="button"
        onclick={() => { activeTab = tab.id; }}
      >
        {tab.label}
      </button>
    {/each}
  </div>

  <!-- Tab panels -->
  <div class="stab-panel">

    {#if activeTab === "overlay"}
      <!-- 悬浮窗 -->
      <div class="sform">
        <div class="sform-section">
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

        <div class="sform-section">
          <div class="sform-section-title">原文字幕</div>
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
        </div>

        <div class="sform-section">
          <div class="sform-section-title">中文字幕</div>
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
        </div>
      </div>

    {:else if activeTab === "shortcuts"}
      <div class="sform">
        <div class="sform-section">
          <div class="sform-section-title">全局快捷键</div>
          <p class="shortcut-tip">点击某一项后直接按键即可修改，按 `Esc` 可取消当前录制。</p>
          <div class="shortcut-list">
            {#each shortcutItems as item}
              <div class="shortcut-row">
                <span>{item.label}</span>
                <button
                  class="shortcut-trigger"
                  class:shortcut-trigger-recording={recordingField === item.field}
                  type="button"
                  onclick={() => { recordingField = item.field; }}
                  onkeydown={(event) => {
                    if (recordingField !== item.field) return;
                    void handleShortcutCapture(item.field, event);
                  }}
                >
                  {#if recordingField === item.field}
                    按下快捷键
                  {:else}
                    {formatShortcut(settings.shortcuts[item.field])}
                  {/if}
                </button>
              </div>
            {/each}
          </div>
        </div>
      </div>

    {:else if activeTab === "models"}
      <!-- 离线模型 -->
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

    {:else if activeTab === "data"}
      <!-- 数据管理 -->
      <div class="sform">
        <div class="sform-section">
          <p class="text-dim text-xs" style="margin-bottom:4px">以下操作均不可逆，请谨慎执行。</p>
          <div class="danger-list">
            <div class="danger-item">
              <div class="danger-item-text">
                <span class="danger-item-label">删除所有模型</span>
                <span class="danger-item-desc">删除已下载的所有离线识别模型，需重新下载才能使用</span>
              </div>
              <button class="btn btn-sm btn-outline btn-danger" type="button" onclick={onDeleteAllModels}>删除</button>
            </div>
            <div class="danger-item">
              <div class="danger-item-text">
                <span class="danger-item-label">删除所有缓存</span>
                <span class="danger-item-desc">删除全部已导入素材、音频及字幕，资源列表将被清空，不影响模型</span>
              </div>
              <button class="btn btn-sm btn-outline btn-danger" type="button" onclick={onClearAllCache}>删除</button>
            </div>
            <div class="danger-item danger-item-solid">
              <div class="danger-item-text">
                <span class="danger-item-label">删除全部数据</span>
                <span class="danger-item-desc">删除所有模型 + 音频字幕缓存，并重置全部设置，彻底清空</span>
              </div>
              <button class="btn btn-sm btn-danger-solid" type="button" onclick={onResetAppData}>删除</button>
            </div>
          </div>
        </div>
      </div>
    {/if}

  </div>
</section>

<style>
  /* ── 页面容器：gap 覆盖，让 tab bar 紧贴顶部 ── */
  .settings-page {
    gap: 0 !important;
  }

  /* ── Tab 导航栏：sticky 固定在 .content 滚动容器顶部 ── */
  .stab-bar {
    display: flex;
    gap: 2px;
    /* 向上抵消 .content 的 24px padding，然后贴顶 */
    position: sticky;
    top: -24px;
    z-index: 10;
    background: var(--bg-base);
    border-bottom: 1px solid var(--border);
    /* 向左右各延伸 28px 以覆盖 .content 的 padding */
    margin: 0 -28px;
    padding: 14px 28px 0;
  }

  .stab-btn {
    position: relative;
    padding: 8px 18px;
    font-size: 0.875rem;
    font-weight: 500;
    color: var(--text-secondary);
    background: none;
    border: none;
    cursor: pointer;
    border-radius: var(--radius-sm) var(--radius-sm) 0 0;
    transition: color 150ms, background 150ms;
    white-space: nowrap;
  }

  .stab-btn:hover {
    color: var(--text-primary);
    background: var(--bg-inset);
  }

  .stab-btn.stab-active {
    color: var(--accent);
    font-weight: 600;
  }

  /* 底部指示线 */
  .stab-btn::after {
    content: "";
    position: absolute;
    bottom: -1px;
    left: 0;
    right: 0;
    height: 2px;
    background: var(--accent);
    border-radius: 2px 2px 0 0;
    opacity: 0;
    transition: opacity 150ms;
  }

  .stab-btn.stab-active::after {
    opacity: 1;
  }

  /* ── 面板内容区 ── */
  .stab-panel {
    padding-top: 20px;
  }

  /* ── 表单区块 ── */
  .sform {
    display: flex;
    flex-direction: column;
    gap: 20px;
    max-width: 480px;
  }

  .sform-section {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .sform-section-title {
    font-size: 0.7rem;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--text-dim);
    padding: 4px 0 2px;
    border-bottom: 1px solid var(--border);
    margin-bottom: 2px;
  }

  .shortcut-tip {
    color: var(--text-dim);
    font-size: 0.8rem;
    margin: 0 0 6px;
  }

  .shortcut-list {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .shortcut-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 10px 12px;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: var(--bg-surface);
  }

  .shortcut-trigger {
    min-width: 132px;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: var(--bg-base);
    color: var(--text-primary);
    padding: 8px 12px;
    font: inherit;
    cursor: pointer;
    transition: border-color 150ms, background 150ms, color 150ms;
  }

  .shortcut-trigger:hover,
  .shortcut-trigger-recording {
    border-color: var(--accent);
    color: var(--accent);
    background: var(--accent-soft);
  }
</style>
