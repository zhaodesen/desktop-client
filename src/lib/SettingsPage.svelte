<script lang="ts">
  import type { AppSettings, ModelInfo, ModelStatus, OverlaySettings, ShortcutSettings, ThemeMode } from "../shared/types";
  import ModelList from "./ModelList.svelte";

  interface Props {
    settings: AppSettings;
    availableModels: ModelInfo[];
    modelsStatusMap: Map<string, ModelStatus>;
    isDownloading: boolean;
    downloadingModelId: string | undefined;
    modelDownloadPercent: number;
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
    onThemeChange: (mode: ThemeMode) => void;
  }

  const {
    settings, availableModels, modelsStatusMap, isDownloading,
    downloadingModelId, modelDownloadPercent,
    modelStatusLabel, modelPathLabel, overlayLocked,
    onOverlayVisibleChange, onOverlayLockToggle, onOverlayStyleChange, onOverlayStyleCommit,
    onDownloadModel, onSelectModel, onDeleteModel,
    onShortcutChange, onShortcutCommit,
    onClearAllCache, onDeleteAllModels, onResetAppData,
    onThemeChange,
  }: Props = $props();

  type TabId = "appearance" | "overlay" | "shortcuts" | "models" | "data";
  type ShortcutField = keyof ShortcutSettings;
  type ThemeOption = {
    id: ThemeMode;
    label: string;
    description: string;
  };

  const tabs: { id: TabId; label: string }[] = [
    { id: "appearance", label: "外观" },
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

  const themeOptions: ThemeOption[] = [
    {
      id: "light",
      label: "浅色",
      description: "界面更明亮，适合白天或高亮环境。",
    },
    {
      id: "dark",
      label: "深色",
      description: "对比更克制，适合长时间阅读和夜间使用。",
    },
    {
      id: "system",
      label: "跟随系统",
      description: "自动同步系统外观，不需要手动切换。",
    },
  ];
  let selectedThemeOption = $derived(
    themeOptions.find((item) => item.id === settings.themeMode) ?? themeOptions[1],
  );

  let activeTab = $state<TabId>("appearance");
  let recordingField = $state<ShortcutField | null>(null);

  /* Sliding tab indicator */
  let stabBarEl = $state<HTMLElement | null>(null);
  let indicatorX = $state(0);
  let indicatorW = $state(0);
  let indicatorRafId = 0;

  function updateIndicator() {
    if (!stabBarEl) return;
    const activeBtn = stabBarEl.querySelector<HTMLElement>('.stab-btn.stab-active');
    if (!activeBtn) return;
    const barRect = stabBarEl.getBoundingClientRect();
    const btnRect = activeBtn.getBoundingClientRect();
    indicatorX = btnRect.left - barRect.left;
    indicatorW = btnRect.width;
  }

  $effect(() => {
    // Explicitly read to establish dependency tracking
    void activeTab;
    cancelAnimationFrame(indicatorRafId);
    indicatorRafId = requestAnimationFrame(updateIndicator);
  });

  const fontSizeDisplay = $derived(`${Math.round(settings.overlay.fontSize)}px`);
  const opacityDisplay = $derived(`${Math.round(settings.overlay.opacity * 100)}%`);
  const opacityPct = $derived(Math.round(settings.overlay.opacity * 100));
  const fontSizeProgress = $derived(
    `${Math.max(0, Math.min(100, ((settings.overlay.fontSize - 24) / (60 - 24)) * 100))}%`,
  );
  const opacityProgress = $derived(
    `${Math.max(0, Math.min(100, ((opacityPct - 60) / (100 - 60)) * 100))}%`,
  );

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
  <div class="stab-bar" bind:this={stabBarEl}>
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
    <span
      class="stab-indicator"
      style="left: {indicatorX}px; width: {indicatorW}px"
    ></span>
  </div>

  <!-- Tab panels -->
  <div class="stab-panel">

    {#if activeTab === "appearance"}
      <!-- 外观 -->
      <div class="sform">
        <div class="sform-section">
          <div class="sform-section-title">主题模式</div>
          <div class="theme-section-copy">
            <h3>选择应用外观</h3>
            <p>切换后会立即应用到当前窗口、侧边栏和播放器相关界面。</p>
          </div>
          <div class="theme-segmented" role="radiogroup" aria-label="主题模式">
            {#each themeOptions as option}
              <button
                class="theme-segment"
                class:theme-segment-active={settings.themeMode === option.id}
                type="button"
                onclick={() => onThemeChange(option.id)}
                aria-pressed={settings.themeMode === option.id}
              >
                <span class="theme-segment-icon" data-mode={option.id}>
                  {#if option.id === "light"}
                    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                      <circle cx="12" cy="12" r="5" />
                      <line x1="12" y1="1" x2="12" y2="3" />
                      <line x1="12" y1="21" x2="12" y2="23" />
                      <line x1="4.22" y1="4.22" x2="5.64" y2="5.64" />
                      <line x1="18.36" y1="18.36" x2="19.78" y2="19.78" />
                      <line x1="1" y1="12" x2="3" y2="12" />
                      <line x1="21" y1="12" x2="23" y2="12" />
                      <line x1="4.22" y1="19.78" x2="5.64" y2="18.36" />
                      <line x1="18.36" y1="5.64" x2="19.78" y2="4.22" />
                    </svg>
                  {:else if option.id === "dark"}
                    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                      <path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z" />
                    </svg>
                  {:else}
                    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                      <rect x="2" y="3" width="20" height="14" rx="2" ry="2" />
                      <line x1="8" y1="21" x2="16" y2="21" />
                      <line x1="12" y1="17" x2="12" y2="21" />
                    </svg>
                  {/if}
                </span>
                <span>{option.label}</span>
              </button>
            {/each}
          </div>

          <div class="theme-preview-panel" data-mode={settings.themeMode}>
            <div class="theme-preview-panel-top">
              <div class="theme-preview-panel-copy">
                <span class="theme-preview-panel-label">当前模式</span>
                <strong>{selectedThemeOption.label}</strong>
              </div>
              <span class="theme-preview-panel-badge">已启用</span>
            </div>

            <div class="theme-window-preview" data-mode={settings.themeMode} aria-hidden="true">
              <div class="theme-window-preview-header">
                <span></span>
                <span></span>
                <span></span>
              </div>
              <div class="theme-window-preview-body">
                <div class="theme-window-preview-sidebar">
                  <span class="theme-window-preview-nav"></span>
                  <span class="theme-window-preview-nav"></span>
                  <span class="theme-window-preview-nav theme-window-preview-nav-active"></span>
                  <span class="theme-window-preview-nav"></span>
                </div>
                <div class="theme-window-preview-main">
                  <span class="theme-window-preview-chip"></span>
                  <span class="theme-window-preview-title"></span>
                  <span class="theme-window-preview-line"></span>
                  <span class="theme-window-preview-line theme-window-preview-line-short"></span>
                  <div class="theme-window-preview-cards">
                    <span></span>
                    <span></span>
                    <span></span>
                  </div>
                </div>
              </div>
            </div>

            <p class="theme-preview-description">{selectedThemeOption.description}</p>
          </div>

          <div class="theme-current-note">
            系统模式会自动跟随设备主题变化；浅色和深色则固定使用当前选择。
          </div>
        </div>
      </div>

    {:else if activeTab === "overlay"}
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
                style={`--range-progress: ${fontSizeProgress};`}
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
                style={`--range-progress: ${opacityProgress};`}
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
        {downloadingModelId}
        {modelDownloadPercent}
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
  /* ── 页面容器 ── */
  .settings-page {
    gap: 0 !important;
  }

  /* ── Tab 导航栏 ── */
  .stab-bar {
    display: flex;
    gap: 2px;
    position: sticky;
    top: -24px;
    z-index: var(--z-sticky);
    background: var(--bg-base);
    border-bottom: 1px solid var(--border);
    margin: 0 -32px;
    padding: 14px 32px 0;
  }

  .stab-btn {
    position: relative;
    padding: 9px 18px;
    font-size: var(--font-base);
    font-weight: 500;
    color: var(--text-secondary);
    background: none;
    border: none;
    cursor: pointer;
    border-radius: var(--radius-sm) var(--radius-sm) 0 0;
    transition: color var(--transition-fast), background var(--transition-fast);
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

  /* Sliding indicator element — uses transform for GPU compositing */
  .stab-indicator {
    position: absolute;
    bottom: -1px;
    height: 2px;
    background: var(--accent);
    border-radius: 2px 2px 0 0;
    transition: left var(--transition-smooth), width var(--transition-smooth);
    pointer-events: none;
    box-shadow: 0 0 8px rgba(var(--accent-rgb), 0.3);
  }

  /* ── 面板内容区 ── */
  .stab-panel {
    padding-top: 24px;
  }

  /* ── 表单区块 ── */
  .sform {
    display: flex;
    flex-direction: column;
    gap: 24px;
    max-width: 520px;
  }

  .sform-section {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .sform-section-title {
    font-size: var(--font-2xs);
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--text-dim);
    padding: 6px 0 4px;
    border-bottom: 1px solid var(--border-subtle);
    margin-bottom: 4px;
  }

  .shortcut-tip {
    color: var(--text-dim);
    font-size: var(--font-xs);
    margin: 0 0 8px;
    line-height: 1.5;
  }

  .shortcut-list {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .shortcut-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 10px 14px;
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-md);
    background: var(--bg-surface);
    transition: border-color var(--transition-fast);
  }

  .shortcut-row:hover {
    border-color: var(--border);
  }

  .shortcut-trigger {
    min-width: 132px;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: var(--bg-base);
    color: var(--text-primary);
    padding: 8px 12px;
    font: inherit;
    font-size: var(--font-sm);
    cursor: pointer;
    text-align: center;
    transition: border-color var(--transition-fast), background var(--transition-fast), color var(--transition-fast), box-shadow var(--transition-fast);
  }

  .shortcut-trigger:hover {
    border-color: var(--border-focus);
  }

  .shortcut-trigger-recording {
    border-color: var(--accent);
    color: var(--accent);
    background: var(--accent-soft);
    box-shadow: 0 0 0 3px var(--accent-soft-2);
    animation: pulse-ring 1.5s ease-in-out infinite;
  }

  @keyframes pulse-ring {
    0%, 100% { box-shadow: 0 0 0 3px var(--accent-soft); }
    50% { box-shadow: 0 0 0 6px transparent; }
  }

  .theme-section-copy {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin-top: 6px;
  }

  .theme-section-copy h3 {
    font-size: var(--font-lg);
    letter-spacing: -0.02em;
    color: var(--text-primary);
  }

  .theme-section-copy p {
    font-size: var(--font-sm);
    line-height: 1.65;
    color: var(--text-secondary);
  }

  .theme-segmented {
    display: inline-flex;
    gap: 6px;
    margin-top: 12px;
    padding: 6px;
    border-radius: var(--radius-lg);
    background: var(--bg-inset);
    border: 1px solid var(--border-subtle);
    width: fit-content;
    max-width: 100%;
  }

  .theme-segment {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    padding: 10px 14px;
    border: none;
    border-radius: var(--radius-md);
    background: transparent;
    color: var(--text-secondary);
    font: inherit;
    cursor: pointer;
    white-space: nowrap;
    transition: background var(--transition-fast), color var(--transition-fast), box-shadow var(--transition-fast);
  }

  .theme-segment:hover {
    color: var(--text-primary);
    background: rgba(255, 255, 255, 0.03);
  }

  .theme-segment-active {
    background: var(--bg-raised);
    color: var(--text-primary);
    box-shadow: var(--shadow-sm);
  }

  .theme-segment-icon {
    width: 24px;
    height: 24px;
    border-radius: 999px;
    display: grid;
    place-items: center;
    flex-shrink: 0;
    background: var(--bg-surface);
    border: 1px solid var(--border-subtle);
  }

  .theme-segment-icon[data-mode="light"] {
    color: #f59e0b;
  }

  .theme-segment-icon[data-mode="dark"] {
    color: #8b93ff;
  }

  .theme-segment-icon[data-mode="system"] {
    color: var(--accent);
  }

  .theme-preview-panel {
    display: flex;
    flex-direction: column;
    gap: 14px;
    margin-top: 16px;
    padding: 16px;
    border-radius: 18px;
    border: 1px solid var(--border);
    background: var(--bg-surface);
  }

  .theme-preview-panel[data-mode="light"] {
    background: color-mix(in srgb, var(--bg-surface) 84%, white);
  }

  .theme-preview-panel[data-mode="dark"] {
    background: color-mix(in srgb, var(--bg-surface) 88%, #07090f);
  }

  .theme-preview-panel-top {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }

  .theme-preview-panel-copy {
    display: flex;
    flex-direction: column;
    gap: 3px;
  }

  .theme-preview-panel-label {
    font-size: var(--font-2xs);
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--text-dim);
  }

  .theme-preview-panel-copy strong {
    font-size: var(--font-lg);
    letter-spacing: -0.02em;
    color: var(--text-primary);
  }

  .theme-preview-panel-badge {
    display: inline-flex;
    align-items: center;
    padding: 5px 10px;
    border-radius: var(--radius-pill);
    font-size: var(--font-2xs);
    font-weight: 700;
    background: rgba(var(--accent-rgb), 0.12);
    border: 1px solid rgba(var(--accent-rgb), 0.18);
    color: var(--accent);
  }

  .theme-window-preview {
    border-radius: 16px;
    overflow: hidden;
    border: 1px solid var(--border-subtle);
    box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.03);
  }

  .theme-window-preview[data-mode="light"] {
    background: #fbfbfc;
    border-color: rgba(0, 0, 0, 0.06);
  }

  .theme-window-preview[data-mode="dark"] {
    background: #0f131b;
    border-color: rgba(255, 255, 255, 0.06);
  }

  .theme-window-preview[data-mode="system"] {
    background:
      linear-gradient(90deg, #fbfbfc 0%, #fbfbfc 50%, #0f131b 50%, #0f131b 100%);
    border-color: rgba(var(--accent-rgb), 0.16);
  }

  .theme-window-preview-header {
    display: flex;
    align-items: center;
    gap: 6px;
    height: 22px;
    padding: 0 10px;
  }

  .theme-window-preview-header span {
    width: 7px;
    height: 7px;
    border-radius: 999px;
    background: currentColor;
    opacity: 0.18;
  }

  .theme-window-preview[data-mode="light"] .theme-window-preview-header {
    background: #f0f0f3;
    color: #0f172a;
  }

  .theme-window-preview[data-mode="dark"] .theme-window-preview-header {
    background: #181d28;
    color: #f8fafc;
  }

  .theme-window-preview[data-mode="system"] .theme-window-preview-header {
    background:
      linear-gradient(90deg, #f0f0f3 0%, #f0f0f3 50%, #181d28 50%, #181d28 100%);
    color: #f8fafc;
  }

  .theme-window-preview-body {
    display: grid;
    grid-template-columns: 124px 1fr;
    min-height: 180px;
  }

  .theme-window-preview-sidebar {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 14px 12px;
  }

  .theme-window-preview-nav {
    width: 100%;
    height: 10px;
    border-radius: 999px;
    background: currentColor;
    opacity: 0.12;
  }

  .theme-window-preview-nav-active {
    opacity: 0.4;
  }

  .theme-window-preview[data-mode="light"] .theme-window-preview-sidebar {
    background: #f3f3f6;
    color: #0f172a;
    border-right: 1px solid rgba(0, 0, 0, 0.05);
  }

  .theme-window-preview[data-mode="dark"] .theme-window-preview-sidebar {
    background: #141924;
    color: #f8fafc;
    border-right: 1px solid rgba(255, 255, 255, 0.05);
  }

  .theme-window-preview[data-mode="system"] .theme-window-preview-sidebar {
    background:
      linear-gradient(90deg, #f3f3f6 0%, #f3f3f6 50%, #141924 50%, #141924 100%);
    color: #f8fafc;
    border-right: none;
  }

  .theme-window-preview-main {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 18px;
  }

  .theme-window-preview-chip,
  .theme-window-preview-title,
  .theme-window-preview-line,
  .theme-window-preview-cards span {
    display: block;
    border-radius: 999px;
  }

  .theme-window-preview-chip {
    width: 74px;
    height: 22px;
    background: rgba(var(--accent-rgb), 0.7);
  }

  .theme-window-preview-title {
    width: 52%;
    height: 14px;
  }

  .theme-window-preview-line {
    width: 100%;
    height: 10px;
  }

  .theme-window-preview-line-short {
    width: 66%;
  }

  .theme-window-preview-cards {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 10px;
    margin-top: auto;
  }

  .theme-window-preview-cards span {
    height: 62px;
  }

  .theme-window-preview[data-mode="light"] .theme-window-preview-main {
    background: #ffffff;
    color: #0f172a;
  }

  .theme-window-preview[data-mode="light"] .theme-window-preview-title,
  .theme-window-preview[data-mode="light"] .theme-window-preview-line,
  .theme-window-preview[data-mode="light"] .theme-window-preview-cards span {
    background: rgba(15, 23, 42, 0.12);
  }

  .theme-window-preview[data-mode="dark"] .theme-window-preview-main {
    background: #0f131b;
    color: #f8fafc;
  }

  .theme-window-preview[data-mode="dark"] .theme-window-preview-title,
  .theme-window-preview[data-mode="dark"] .theme-window-preview-line,
  .theme-window-preview[data-mode="dark"] .theme-window-preview-cards span {
    background: rgba(255, 255, 255, 0.1);
  }

  .theme-window-preview[data-mode="system"] .theme-window-preview-main {
    background:
      linear-gradient(90deg, #ffffff 0%, #ffffff 50%, #0f131b 50%, #0f131b 100%);
    color: #f8fafc;
  }

  .theme-window-preview[data-mode="system"] .theme-window-preview-title,
  .theme-window-preview[data-mode="system"] .theme-window-preview-line,
  .theme-window-preview[data-mode="system"] .theme-window-preview-cards span {
    background:
      linear-gradient(90deg, rgba(15, 23, 42, 0.12) 0%, rgba(15, 23, 42, 0.12) 50%, rgba(255, 255, 255, 0.12) 50%, rgba(255, 255, 255, 0.12) 100%);
  }

  .theme-preview-description {
    margin: 0;
    font-size: var(--font-xs);
    line-height: 1.65;
    color: var(--text-dim);
  }

  .theme-current-note {
    margin-top: 4px;
    font-size: var(--font-xs);
    color: var(--text-dim);
  }

  @media (max-width: 720px) {
    .theme-segmented {
      width: 100%;
      display: grid;
      grid-template-columns: 1fr;
    }

    .theme-segment {
      justify-content: center;
    }

    .theme-window-preview-body {
      grid-template-columns: 96px 1fr;
      min-height: 156px;
    }

    .theme-window-preview-cards {
      grid-template-columns: 1fr;
    }
  }
</style>
