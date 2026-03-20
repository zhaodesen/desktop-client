<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import type { ImportProgress } from "../shared/types";

  interface Props {
    progress: ImportProgress;
    importError: string | undefined;
    importSuccessName: string | undefined;
    showSuccess: boolean;
    onImportMedia: () => void;
    onDismissError: () => void;
    onImportSuccessClose: () => void;
    onGoToResources: () => void;
  }

  const {
    progress,
    importError,
    importSuccessName,
    showSuccess,
    onImportMedia,
    onDismissError,
    onImportSuccessClose,
    onGoToResources,
  }: Props = $props();

  const SUPPRESS_KEY = "import_suppress_dialog";

  let suppressDialog = $state(false);
  let dialogSuppressChecked = $state(false);
  let showToast = $state(false);
  let toastTimer: ReturnType<typeof setTimeout> | undefined;

  onMount(() => {
    suppressDialog = localStorage.getItem(SUPPRESS_KEY) === "true";
  });

  onDestroy(() => {
    clearTimeout(toastTimer);
  });

  // 导入成功且已勾选"不再提示" → 显示 toast 替代弹框
  $effect(() => {
    if (showSuccess && suppressDialog) {
      showToast = true;
      clearTimeout(toastTimer);
      toastTimer = setTimeout(() => {
        showToast = false;
        onImportSuccessClose();
      }, 3000);
    }
  });

  const showDialog = $derived(showSuccess && !suppressDialog);

  // 阶段标签映射
  const stageLabels: Record<string, string> = {
    importing: "导入媒体",
    preparing: "检查依赖",
    recognizing: "离线识别",
    translating: "生成翻译",
    done: "全部完成",
  };

  const stageLabel = $derived(stageLabels[progress.stage] ?? progress.stage);

  function saveSuppress() {
    if (dialogSuppressChecked) {
      suppressDialog = true;
      localStorage.setItem(SUPPRESS_KEY, "true");
    }
  }

  function handleDialogClose() {
    saveSuppress();
    dialogSuppressChecked = false;
    onImportSuccessClose();
  }

  function handleGoToResources() {
    saveSuppress();
    dialogSuppressChecked = false;
    onImportSuccessClose();
    onGoToResources();
  }
</script>

<!-- 导入失败：顶部固定错误条 -->
{#if importError}
  <div class="import-error-bar" role="alert">
    <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" style="flex-shrink:0">
      <circle cx="12" cy="12" r="10"/><line x1="12" y1="8" x2="12" y2="12"/><line x1="12" y1="16" x2="12.01" y2="16"/>
    </svg>
    <span class="import-error-msg">{importError}</span>
    <button class="import-error-close" type="button" onclick={onDismissError} aria-label="关闭错误">
      <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
        <line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/>
      </svg>
    </button>
  </div>
{/if}

<!-- 页面主体 -->
<section class="page import-page" data-active="true">
  {#if progress.active}
    <!-- 导入进度 -->
    <div class="import-center">
      <div class="import-progress-icon">
        {#if progress.stage === "done"}
          <svg width="36" height="36" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
            <polyline points="20 6 9 17 4 12"/>
          </svg>
        {:else}
          <div class="import-spinner"></div>
        {/if}
      </div>
      <h2 class="import-title">{stageLabel}</h2>
      <p class="import-desc">{progress.message}</p>

      <!-- 进度条 -->
      <div class="progress-bar-wrapper">
        <div class="progress-bar-track">
          <div
            class="progress-bar-fill"
            class:progress-done={progress.stage === "done"}
            style="width: {progress.percent}%"
          ></div>
        </div>
        <span class="progress-percent">{Math.round(progress.percent)}%</span>
      </div>

      <!-- 阶段指示器 -->
      <div class="progress-stages">
        <span class="stage-dot" class:stage-active={progress.stage === "importing"} class:stage-done={["preparing","recognizing","translating","done"].includes(progress.stage)}>导入</span>
        <span class="stage-line"></span>
        <span class="stage-dot" class:stage-active={progress.stage === "preparing"} class:stage-done={["recognizing","translating","done"].includes(progress.stage)}>检查</span>
        <span class="stage-line"></span>
        <span class="stage-dot" class:stage-active={progress.stage === "recognizing"} class:stage-done={["translating","done"].includes(progress.stage)}>识别</span>
        <span class="stage-line"></span>
        <span class="stage-dot" class:stage-active={progress.stage === "translating"} class:stage-done={progress.stage === "done"}>翻译</span>
        <span class="stage-line"></span>
        <span class="stage-dot" class:stage-active={progress.stage === "done"} class:stage-done={false}>完成</span>
      </div>
    </div>
  {:else}
    <!-- 默认状态 -->
    <div class="import-center">
      <div class="import-icon">
        <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
          <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/>
          <polyline points="17 8 12 3 7 8"/>
          <line x1="12" y1="3" x2="12" y2="15"/>
        </svg>
      </div>
      <h2 class="import-title">导入本地媒体</h2>
      <p class="import-desc">
        支持导入本地视频或音频文件，自动提取音频并离线生成双语字幕（含中文翻译）。
      </p>
      <p class="import-formats">
        视频：MP4、MOV、MKV、WebM、AVI &nbsp;·&nbsp; 音频：MP3、WAV、M4A、AAC、FLAC、OGG
      </p>
      <button class="btn btn-primary btn-lg" type="button" onclick={onImportMedia}>
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/>
        </svg>
        选择文件导入
      </button>
    </div>
  {/if}
</section>

<!-- Toast（不再提示模式） -->
{#if showToast}
  <div class="import-toast" role="status">
    <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
      <polyline points="20 6 9 17 4 12"/>
    </svg>
    <span>全部完成，双语字幕已生成</span>
  </div>
{/if}

<!-- 导入成功弹框 -->
{#if showDialog}
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <div class="import-dialog-backdrop" role="presentation" onclick={handleDialogClose}></div>
  <div class="import-dialog" role="dialog" aria-modal="true">
    <div class="import-dialog-check">
      <svg width="26" height="26" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
        <polyline points="20 6 9 17 4 12"/>
      </svg>
    </div>
    <h3 class="import-dialog-title">导入成功</h3>
    <p class="import-dialog-desc">
      {#if importSuccessName}
        「{importSuccessName}」已导入，双语字幕已生成完毕。
      {:else}
        媒体文件已导入，双语字幕已生成完毕。
      {/if}
    </p>
    <label class="import-dialog-suppress">
      <input type="checkbox" bind:checked={dialogSuppressChecked} />
      <span>不再提示</span>
    </label>
    <div class="import-dialog-actions">
      <button class="btn btn-ghost btn-sm" type="button" onclick={handleDialogClose}>关闭</button>
      <button class="btn btn-primary btn-sm" type="button" onclick={handleGoToResources}>去查看</button>
    </div>
  </div>
{/if}

<style>
  /* ── 错误条 ── */
  .import-error-bar {
    position: sticky;
    top: 0;
    z-index: 10;
    display: flex;
    align-items: flex-start;
    gap: 10px;
    padding: 12px 16px;
    background: var(--danger-soft);
    border: 1px solid var(--danger-border);
    border-radius: var(--radius-sm);
    color: var(--danger);
    font-size: 0.83rem;
    line-height: 1.5;
    margin-bottom: 12px;
  }

  .import-error-msg {
    flex: 1;
    min-width: 0;
    word-break: break-all;
  }

  .import-error-close {
    display: grid;
    place-items: center;
    width: 22px;
    height: 22px;
    border: none;
    background: transparent;
    color: var(--danger);
    cursor: pointer;
    border-radius: 4px;
    flex-shrink: 0;
    margin-top: -1px;
    opacity: 0.75;
    transition: opacity 150ms;
  }
  .import-error-close:hover { opacity: 1; }

  /* ── 页面主体 ── */
  .import-page {
    display: flex !important;
    flex-direction: column;
  }

  .import-center {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    text-align: center;
    min-height: 65vh;
    gap: 12px;
    padding: 24px;
  }

  .import-icon {
    width: 80px;
    height: 80px;
    display: grid;
    place-items: center;
    border-radius: 20px;
    background: var(--accent-soft);
    color: var(--accent);
    margin-bottom: 8px;
  }

  .import-title {
    font-size: 1.3rem;
    font-weight: 700;
    color: var(--text-primary);
  }

  .import-desc {
    font-size: 0.9rem;
    color: var(--text-secondary);
    max-width: 420px;
    line-height: 1.6;
  }

  .import-formats {
    font-size: 0.75rem;
    color: var(--text-dim);
    max-width: 420px;
  }

  :global(.btn-lg) {
    padding: 12px 28px;
    font-size: 0.95rem;
    border-radius: var(--radius-md);
    margin-top: 8px;
  }

  /* ── Progress icon ── */
  .import-progress-icon {
    width: 80px;
    height: 80px;
    display: grid;
    place-items: center;
    border-radius: 20px;
    background: var(--accent-soft);
    color: var(--accent);
    margin-bottom: 8px;
  }

  .import-spinner {
    width: 36px;
    height: 36px;
    border: 3px solid rgba(232, 148, 78, 0.25);
    border-top-color: var(--accent);
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }

  /* ── Progress bar ── */
  .progress-bar-wrapper {
    display: flex;
    align-items: center;
    gap: 12px;
    width: 100%;
    max-width: 360px;
    margin-top: 8px;
  }

  .progress-bar-track {
    flex: 1;
    height: 6px;
    background: var(--bg-inset, rgba(255, 255, 255, 0.06));
    border-radius: 3px;
    overflow: hidden;
  }

  .progress-bar-fill {
    height: 100%;
    background: var(--accent);
    border-radius: 3px;
    transition: width 0.5s ease;
  }

  .progress-bar-fill.progress-done {
    background: var(--success, #4ade80);
  }

  .progress-percent {
    font-size: 0.8rem;
    font-weight: 600;
    color: var(--text-secondary);
    min-width: 36px;
    text-align: right;
    font-variant-numeric: tabular-nums;
  }

  /* ── Stage indicators ── */
  .progress-stages {
    display: flex;
    align-items: center;
    gap: 0;
    margin-top: 12px;
  }

  .stage-dot {
    font-size: 0.72rem;
    color: var(--text-dim);
    padding: 3px 8px;
    border-radius: var(--radius-pill, 999px);
    border: 1px solid transparent;
    transition: all 0.25s ease;
  }

  .stage-dot.stage-active {
    color: var(--accent);
    background: var(--accent-soft);
    border-color: var(--accent);
    font-weight: 600;
  }

  .stage-dot.stage-done {
    color: var(--success, #4ade80);
  }

  .stage-line {
    width: 16px;
    height: 1px;
    background: var(--border);
  }

  /* ── Toast ── */
  .import-toast {
    position: fixed;
    bottom: 28px;
    left: 50%;
    transform: translateX(-50%);
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 18px;
    background: var(--bg-surface);
    border: 1px solid var(--border);
    border-radius: var(--radius-pill);
    color: var(--success);
    font-size: 0.83rem;
    font-weight: 500;
    box-shadow: 0 4px 20px rgba(0, 0, 0, 0.35);
    z-index: 100;
    animation: toast-in 0.2s ease;
  }

  @keyframes toast-in {
    from { opacity: 0; transform: translateX(-50%) translateY(8px); }
    to   { opacity: 1; transform: translateX(-50%) translateY(0); }
  }

  /* ── 成功弹框 ── */
  .import-dialog-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.5);
    backdrop-filter: blur(2px);
    z-index: 200;
  }

  .import-dialog {
    position: fixed;
    top: 50%;
    left: calc(var(--sidebar-w, 220px) + (100vw - var(--sidebar-w, 220px)) / 2);
    transform: translate(-50%, -50%);
    width: 340px;
    background: var(--bg-raised);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    padding: 28px 24px 20px;
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
    gap: 8px;
    z-index: 201;
    box-shadow: 0 16px 48px rgba(0, 0, 0, 0.5);
    animation: dialog-in 0.18s ease;
  }

  @keyframes dialog-in {
    from { opacity: 0; transform: translate(-50%, calc(-50% + 10px)); }
    to   { opacity: 1; transform: translate(-50%, -50%); }
  }

  .import-dialog-check {
    width: 56px;
    height: 56px;
    border-radius: 50%;
    background: var(--success-soft);
    color: var(--success);
    display: grid;
    place-items: center;
    margin-bottom: 4px;
  }

  .import-dialog-title {
    font-size: 1.05rem;
    font-weight: 700;
    color: var(--text-primary);
  }

  .import-dialog-desc {
    font-size: 0.83rem;
    color: var(--text-secondary);
    line-height: 1.55;
    max-width: 280px;
    margin-top: 2px;
  }

  .import-dialog-suppress {
    display: flex;
    align-items: center;
    gap: 7px;
    font-size: 0.8rem;
    color: var(--text-dim);
    cursor: pointer;
    margin-top: 8px;
    user-select: none;
  }
  .import-dialog-suppress input { cursor: pointer; accent-color: var(--accent); }
  .import-dialog-suppress:hover { color: var(--text-secondary); }

  .import-dialog-actions {
    display: flex;
    gap: 8px;
    margin-top: 8px;
    width: 100%;
    justify-content: flex-end;
  }
</style>
