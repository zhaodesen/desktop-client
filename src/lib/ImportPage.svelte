<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import type { ImportProgress } from "../shared/types";

  interface Props {
    progress: ImportProgress;
    importError: string | undefined;
    importSuccessName: string | undefined;
    showSuccess: boolean;
    canCancel: boolean;
    isCancellingAsr: boolean;
    onImportMedia: () => void;
    onImportOnline: (url: string) => Promise<void> | void;
    onCancel: () => void;
    onDismissError: () => void;
    onImportSuccessClose: () => void;
    onGoToResources: () => void;
  }

  const {
    progress,
    importError,
    importSuccessName,
    showSuccess,
    canCancel,
    isCancellingAsr,
    onImportMedia,
    onImportOnline,
    onCancel,
    onDismissError,
    onImportSuccessClose,
    onGoToResources,
  }: Props = $props();

  const SUPPRESS_KEY = "import_suppress_dialog";

  let suppressDialog = $state(false);
  let dialogSuppressChecked = $state(false);
  let showToast = $state(false);
  let showOnlineDialog = $state(false);
  let onlineUrl = $state("");
  let onlineUrlError = $state<string | undefined>(undefined);
  let isSubmittingOnline = $state(false);
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
    downloading: "下载在线视频",
    importing: "导入媒体",
    preparing: "检查依赖",
    recognizing: "离线识别",
    translating: "生成翻译",
    done: "全部完成",
  };

  const stageLabel = $derived(stageLabels[progress.stage] ?? progress.stage);
  const importErrorHint = $derived(getImportErrorHint(importError));

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

  function getImportErrorHint(error: string | undefined): string | undefined {
    if (!error) return undefined;

    const lowered = error.toLowerCase();
    const isBilibiliError =
      lowered.includes("bilibili")
      || lowered.includes("b23.tv")
      || error.includes("B 站")
      || error.includes("浏览器 Cookie");

    if (!isBilibiliError) return undefined;

    return "可先在本机浏览器里登录一次 B 站，再回到这里重试。应用会自动尝试读取浏览器 Cookie。";
  }

  async function handleOnlineImportSubmit() {
    const trimmedUrl = onlineUrl.trim();
    if (!trimmedUrl) {
      onlineUrlError = "请输入在线视频地址";
      return;
    }

    isSubmittingOnline = true;
    onlineUrlError = undefined;
    showOnlineDialog = false;
    onlineUrl = "";
    try {
      await onImportOnline(trimmedUrl);
    } catch {
      // 具体错误由父组件统一显示在导入页顶部错误条。
    } finally {
      isSubmittingOnline = false;
    }
  }

  function handleCloseOnlineDialog() {
    if (isSubmittingOnline) return;
    showOnlineDialog = false;
    onlineUrlError = undefined;
  }
</script>

<!-- 导入失败：顶部固定错误条 -->
{#if importError}
  <div class="import-error-bar" role="alert">
    <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" style="flex-shrink:0">
      <circle cx="12" cy="12" r="10"/><line x1="12" y1="8" x2="12" y2="12"/><line x1="12" y1="16" x2="12.01" y2="16"/>
    </svg>
    <div class="import-error-copy">
      <span class="import-error-msg">{importError}</span>
      {#if importErrorHint}
        <div class="import-error-hint">{importErrorHint}</div>
      {/if}
    </div>
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
            class:progress-active={progress.stage !== "done"}
            style="width: {progress.percent}%"
          ></div>
        </div>
        <span class="progress-percent">{Math.round(progress.percent)}%</span>
      </div>

      <!-- 阶段指示器 -->
      <div class="progress-stages">
        <span class="stage-dot" class:stage-active={progress.stage === "downloading"} class:stage-done={["importing","preparing","recognizing","translating","done"].includes(progress.stage)}>下载</span>
        <span class="stage-line"></span>
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
      {#if canCancel}
        <button class="btn btn-ghost btn-sm import-cancel-btn" type="button" onclick={onCancel} disabled={isCancellingAsr}>
          {#if isCancellingAsr}正在取消…{:else}取消识别{/if}
        </button>
      {/if}
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
      <button class="btn btn-outline btn-lg import-online-btn" type="button" onclick={() => { showOnlineDialog = true; }}>
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <path d="M5 12h14"/><path d="M12 5l7 7-7 7"/><path d="M5 5h.01"/><path d="M5 19h.01"/>
        </svg>
        导入在线视频
      </button>
      <p class="import-online-hint">
        适合直接抓取支持的网站视频链接。不支持的地址建议先手动下载到本地，再用上方按钮导入。
      </p>
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

{#if showOnlineDialog}
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <div class="import-dialog-backdrop" role="presentation" onclick={handleCloseOnlineDialog}></div>
  <div class="import-dialog import-online-dialog" role="dialog" aria-modal="true">
    <div class="import-dialog-check import-dialog-check-accent">
      <svg width="26" height="26" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.1" stroke-linecap="round" stroke-linejoin="round">
        <path d="M5 12h14" /><path d="M12 5l7 7-7 7" />
      </svg>
    </div>
    <h3 class="import-dialog-title">导入在线视频</h3>
    <p class="import-dialog-desc">
      粘贴在线视频地址后，应用会通过 <code>yt-dlp</code> 下载原视频到你电脑的下载目录，再自动导入本应用继续生成字幕。
    </p>
    <p class="import-dialog-tip">
      部分站点、私有链接或带校验参数的 URL 可能不支持。如果下载失败，建议先手动保存到本地后再导入。
    </p>

    <label class="online-input-block">
      <span>视频网址</span>
      <input
        type="url"
        placeholder="https://example.com/watch?v=..."
        bind:value={onlineUrl}
        disabled={isSubmittingOnline}
      />
    </label>

    {#if onlineUrlError}
      <div class="online-input-error">{onlineUrlError}</div>
    {/if}

    <div class="import-dialog-actions">
      <button class="btn btn-ghost btn-sm" type="button" onclick={handleCloseOnlineDialog} disabled={isSubmittingOnline}>取消</button>
      <button class="btn btn-primary btn-sm" type="button" onclick={() => { void handleOnlineImportSubmit(); }} disabled={isSubmittingOnline}>
        开始下载并导入
      </button>
    </div>
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
    border-radius: var(--radius-md);
    color: var(--danger);
    font-size: 0.83rem;
    line-height: 1.5;
    margin-bottom: 12px;
  }

  .import-error-msg { min-width: 0; word-break: break-all; }
  .import-error-copy { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 6px; }

  .import-error-hint {
    padding: 8px 10px;
    border-radius: var(--radius-sm);
    background: var(--danger-soft);
    border: 1px solid var(--danger-border);
    color: var(--danger);
    font-size: 0.8rem;
    line-height: 1.45;
    word-break: break-word;
  }

  .import-error-close {
    display: grid; place-items: center;
    width: 22px; height: 22px;
    border: none; background: transparent;
    color: var(--danger); cursor: pointer;
    border-radius: 4px; flex-shrink: 0;
    margin-top: -1px; opacity: 0.75;
    transition: opacity 150ms;
  }
  .import-error-close:hover { opacity: 1; }

  /* ── 页面主体 ── */
  .import-page { display: flex !important; flex-direction: column; }

  .import-center {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    text-align: center;
    min-height: 65vh;
    gap: 16px;
    padding: 28px;
  }

  .import-icon {
    width: 80px; height: 80px;
    display: grid; place-items: center;
    border-radius: var(--radius-xl, 20px);
    background: var(--accent-soft);
    color: var(--accent);
    margin-bottom: 6px;
    box-shadow: var(--shadow-glow);
    position: relative;
  }

  /* Subtle ring */
  .import-icon::after {
    content: "";
    position: absolute;
    inset: -4px;
    border-radius: inherit;
    border: 1.5px solid var(--accent-border);
    opacity: 0.5;
  }

  .import-title { font-size: 1.3rem; font-weight: 700; color: var(--text-primary); letter-spacing: -0.01em; }
  .import-desc { font-size: 0.88rem; color: var(--text-secondary); max-width: 440px; line-height: 1.7; }
  .import-formats {
    font-size: 0.74rem; color: var(--text-dim); max-width: 440px;
    padding: 8px 16px;
    border-radius: var(--radius-pill);
    background: var(--bg-inset);
    border: 1px solid var(--border-subtle);
  }

  :global(.btn-lg) {
    padding: 11px 28px;
    font-size: 0.92rem;
    border-radius: var(--radius-md);
    margin-top: 6px;
  }

  .import-online-btn { margin-top: 0; }
  .import-online-hint { max-width: 460px; font-size: 0.78rem; color: var(--text-dim); line-height: 1.7; }

  /* ── Progress icon ── */
  .import-progress-icon {
    width: 80px; height: 80px;
    display: grid; place-items: center;
    border-radius: var(--radius-xl, 20px);
    background: var(--accent-soft);
    color: var(--accent);
    margin-bottom: 6px;
    box-shadow: var(--shadow-glow);
  }

  .import-spinner {
    width: 34px; height: 34px;
    border: 3px solid var(--accent-soft);
    border-top-color: var(--accent);
    border-right-color: var(--accent);
    border-radius: 50%;
    animation: spin 0.7s cubic-bezier(0.5, 0.1, 0.5, 0.9) infinite;
    filter: drop-shadow(0 0 4px rgba(224, 149, 69, 0.3));
  }

  @keyframes spin { to { transform: rotate(360deg); } }

  /* ── Progress bar ── */
  .progress-bar-wrapper {
    display: flex; align-items: center; gap: 12px;
    width: 100%; max-width: 380px; margin-top: 10px;
  }

  .progress-bar-track {
    flex: 1; height: 6px;
    background: var(--bg-inset);
    border-radius: 3px; overflow: hidden;
    box-shadow: inset 0 1px 2px rgba(0, 0, 0, 0.1);
  }

  .progress-bar-fill {
    height: 100%; background: var(--accent);
    border-radius: 3px; transition: width 0.5s cubic-bezier(0.4, 0, 0.2, 1);
    position: relative;
  }

  .progress-bar-fill.progress-done {
    background: var(--success);
    box-shadow: 0 0 8px rgba(52, 211, 153, 0.4);
  }

  .progress-bar-fill.progress-active {
    background-image: linear-gradient(90deg, var(--accent) 0%, var(--accent-hover) 50%, var(--accent) 100%);
    background-size: 200% 100%;
    animation: shimmer 1.8s ease-in-out infinite;
    box-shadow: 0 0 8px rgba(224, 149, 69, 0.3);
  }

  /* Leading edge glow on active progress */
  .progress-bar-fill.progress-active::after {
    content: "";
    position: absolute;
    right: 0;
    top: -1px;
    bottom: -1px;
    width: 24px;
    background: linear-gradient(90deg, transparent, rgba(224, 149, 69, 0.6));
    border-radius: 0 3px 3px 0;
    filter: blur(2px);
  }

  @keyframes shimmer {
    0%   { background-position: 200% 0; }
    100% { background-position: -200% 0; }
  }

  .progress-percent {
    font-size: 0.82rem; font-weight: 600;
    color: var(--text-secondary);
    min-width: 36px; text-align: right;
    font-variant-numeric: tabular-nums;
  }

  /* ── Stage indicators ── */
  .progress-stages { display: flex; align-items: center; gap: 0; margin-top: 14px; }
  .import-cancel-btn { margin-top: 16px; min-width: 112px; }

  .stage-dot {
    font-size: 0.72rem; color: var(--text-dim);
    padding: 4px 10px; border-radius: var(--radius-pill);
    border: 1px solid transparent;
    transition: all 0.25s ease;
  }
  .stage-dot.stage-active { color: var(--accent); background: var(--accent-soft); border-color: var(--accent-border); font-weight: 600; }
  .stage-dot.stage-done { color: var(--success); }
  .stage-line { width: 18px; height: 1px; background: var(--border); }

  /* ── Toast ── */
  .import-toast {
    position: fixed; bottom: 28px; left: 50%;
    transform: translateX(-50%);
    display: flex; align-items: center; gap: 8px;
    padding: 10px 20px;
    background: var(--bg-glass);
    backdrop-filter: blur(12px);
    -webkit-backdrop-filter: blur(12px);
    border: 1px solid var(--border);
    border-radius: var(--radius-pill);
    color: var(--success);
    font-size: 0.83rem; font-weight: 500;
    box-shadow: var(--shadow-md);
    z-index: 100;
    animation: toast-in 0.25s ease;
  }

  @keyframes toast-in {
    from { opacity: 0; transform: translateX(-50%) translateY(8px); }
    to   { opacity: 1; transform: translateX(-50%) translateY(0); }
  }

  /* ── 成功弹框 ── */
  .import-dialog-backdrop {
    position: fixed; inset: 0;
    background: rgba(0, 0, 0, 0.45);
    backdrop-filter: blur(6px);
    -webkit-backdrop-filter: blur(6px);
    z-index: 200;
  }

  .import-dialog {
    position: fixed;
    top: 50%;
    left: calc(var(--sidebar-w, 220px) + (100vw - var(--sidebar-w, 220px)) / 2);
    transform: translate(-50%, -50%);
    width: 360px;
    background: var(--bg-raised);
    border: 1px solid var(--border);
    border-radius: var(--radius-xl, 20px);
    padding: 32px 28px 22px;
    display: flex; flex-direction: column;
    align-items: center; text-align: center; gap: 10px;
    z-index: 201;
    box-shadow: var(--shadow-lg);
    animation: dialog-in 0.2s ease;
  }

  @keyframes dialog-in {
    from { opacity: 0; transform: translate(-50%, calc(-50% + 12px)); }
    to   { opacity: 1; transform: translate(-50%, -50%); }
  }

  .import-dialog-check {
    width: 56px; height: 56px;
    border-radius: 50%;
    background: var(--success-soft); color: var(--success);
    display: grid; place-items: center; margin-bottom: 4px;
  }

  .import-dialog-title { font-size: 1.05rem; font-weight: 700; color: var(--text-primary); }
  .import-dialog-desc { font-size: 0.83rem; color: var(--text-secondary); line-height: 1.55; max-width: 290px; margin-top: 2px; }

  .import-dialog-suppress {
    display: flex; align-items: center; gap: 7px;
    font-size: 0.8rem; color: var(--text-dim);
    cursor: pointer; margin-top: 8px; user-select: none;
  }
  .import-dialog-suppress input { cursor: pointer; accent-color: var(--accent); }
  .import-dialog-suppress:hover { color: var(--text-secondary); }

  .import-dialog-actions {
    display: flex; gap: 8px; margin-top: 10px;
    width: 100%; justify-content: flex-end;
  }

  .import-online-dialog { width: 460px; align-items: stretch; text-align: left; }
  .import-dialog-check-accent { background: var(--accent-soft); color: var(--accent); align-self: center; }

  .import-dialog-tip {
    font-size: 0.78rem; line-height: 1.65;
    color: var(--text-dim);
    background: var(--bg-inset);
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-md); padding: 12px 14px;
  }

  .online-input-block {
    display: flex; flex-direction: column; gap: 8px;
    margin-top: 4px; font-size: 0.8rem; color: var(--text-secondary);
  }

  .online-input-block input {
    width: 100%;
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    background: var(--bg-inset);
    color: var(--text-primary);
    font: inherit; padding: 12px 14px;
    outline: none;
    transition: border-color 150ms ease, background 150ms ease, box-shadow 150ms ease;
  }

  .online-input-block input:focus {
    border-color: var(--accent);
    background: var(--bg-surface);
    box-shadow: 0 0 0 3px var(--accent-soft);
  }

  .online-input-error { color: var(--danger); font-size: 0.78rem; margin-top: -2px; }
</style>
