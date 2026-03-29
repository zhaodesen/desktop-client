<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import type { ImportProgress, ImportProgressStage } from "../shared/types";

  interface Props {
    progress: ImportProgress;
    importError: string | undefined;
    importSuccessName: string | undefined;
    showSuccess: boolean;
    canCancel: boolean;
    isCancellingAsr: boolean;
    onImportMedia: () => Promise<void> | void;
    onImportOnline: (url: string) => Promise<void> | void;
    onCancel: () => void;
    onDismissError: () => void;
    onImportSuccessClose: () => void;
    onGoToResources: () => void;
  }

  type ImportViewState = { type: "default" } | { type: "online"; url: string };
  type ImportStageMeta = {
    id: ImportProgressStage;
    short: string;
    label: string;
    description: string;
  };

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

  const IMPORT_STAGES: ImportStageMeta[] = [
    { id: "downloading", short: "下载", label: "下载在线视频", description: "从链接拉取媒体到本机。" },
    { id: "importing", short: "导入", label: "整理媒体素材", description: "写入资源库并准备处理。" },
    { id: "preparing", short: "检查", label: "检查模型与依赖", description: "确认识别环境已经就绪。" },
    { id: "recognizing", short: "识别", label: "离线识别字幕", description: "在本机完成语音转写。" },
    { id: "translating", short: "翻译", label: "生成中文字幕", description: "继续生成中文翻译字幕。" },
    { id: "done", short: "完成", label: "导入处理完成", description: "素材已经可以去资源列表查看。" },
  ];

  let suppressDialog = $state(false);
  let dialogSuppressChecked = $state(false);
  let showToast = $state(false);
  let showOnlineDialog = $state(false);
  let onlineUrl = $state("");
  let onlineUrlError = $state<string | undefined>(undefined);
  let isSubmittingOnline = $state(false);
  let importViewBeforeStart = $state<ImportViewState | undefined>(undefined);
  let hadActiveImport = $state(false);
  let toastTimer: ReturnType<typeof setTimeout> | undefined;

  let showDialog = $derived(showSuccess && !suppressDialog);
  let activeStageIndex = $derived(
    Math.max(0, IMPORT_STAGES.findIndex((item) => item.id === progress.stage)),
  );
  let activeStageMeta = $derived(IMPORT_STAGES[activeStageIndex] ?? IMPORT_STAGES[0]);
  let stageLabel = $derived(activeStageMeta.label);
  let normalizedPercent = $derived(
    Math.max(0, Math.min(100, Number.isFinite(progress.percent) ? progress.percent : 0)),
  );
  let progressSummary = $derived(progress.message || activeStageMeta.description);
  let importErrorHint = $derived(getImportErrorHint(importError));

  onMount(() => {
    suppressDialog = localStorage.getItem(SUPPRESS_KEY) === "true";
  });

  onDestroy(() => {
    clearTimeout(toastTimer);
  });

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

  $effect(() => {
    if (progress.active) {
      hadActiveImport = true;
      return;
    }

    if (!hadActiveImport) return;
    hadActiveImport = false;

    const restoreState = importViewBeforeStart;
    importViewBeforeStart = undefined;
    if (!importError || !restoreState) return;

    showOnlineDialog = restoreState.type === "online";
    onlineUrl = restoreState.type === "online" ? restoreState.url : "";
    onlineUrlError = undefined;
  });

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

  function saveSuppress() {
    if (!dialogSuppressChecked) return;
    suppressDialog = true;
    localStorage.setItem(SUPPRESS_KEY, "true");
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

  function openOnlineDialog() {
    showOnlineDialog = true;
    onlineUrlError = undefined;
  }

  function getStageState(index: number): "done" | "active" | "pending" {
    if (progress.stage === "done") {
      return index === IMPORT_STAGES.length - 1 ? "active" : "done";
    }

    if (index < activeStageIndex) return "done";
    if (index === activeStageIndex) return "active";
    return "pending";
  }

  async function handleLocalImport() {
    importViewBeforeStart = { type: "default" };
    try {
      await onImportMedia();
      importViewBeforeStart = undefined;
    } catch {
      // 具体错误由父组件统一显示在导入页顶部错误条。
    }
  }

  async function handleOnlineImportSubmit() {
    const trimmedUrl = onlineUrl.trim();
    if (!trimmedUrl) {
      onlineUrlError = "请输入在线视频地址";
      return;
    }

    isSubmittingOnline = true;
    onlineUrlError = undefined;
    importViewBeforeStart = { type: "online", url: trimmedUrl };
    showOnlineDialog = false;
    onlineUrl = "";
    try {
      await onImportOnline(trimmedUrl);
      importViewBeforeStart = undefined;
    } catch {
      // 具体错误由父组件统一显示在导入页顶部错误条。
    } finally {
      isSubmittingOnline = false;
    }
  }

  function handleOnlineInputKeydown(event: KeyboardEvent) {
    if (event.key !== "Enter" || isSubmittingOnline) return;
    event.preventDefault();
    void handleOnlineImportSubmit();
  }

  function handleCloseOnlineDialog() {
    if (isSubmittingOnline) return;
    showOnlineDialog = false;
    onlineUrlError = undefined;
  }
</script>

{#if importError}
  <div class="import-error-bar" role="alert">
    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
      <circle cx="12" cy="12" r="10" />
      <line x1="12" y1="8" x2="12" y2="12" />
      <line x1="12" y1="16" x2="12.01" y2="16" />
    </svg>
    <div class="import-error-copy">
      <span class="import-error-msg">{importError}</span>
      {#if importErrorHint}
        <div class="import-error-hint">{importErrorHint}</div>
      {/if}
    </div>
    <button class="import-error-close" type="button" onclick={onDismissError} aria-label="关闭错误">
      <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
        <line x1="18" y1="6" x2="6" y2="18" />
        <line x1="6" y1="6" x2="18" y2="18" />
      </svg>
    </button>
  </div>
{/if}

<section class="page import-page" data-active="true">
  <div class="import-shell">
    {#if progress.active}
      <article class="import-card import-progress-card" role="status" aria-live="polite">
        <div class="import-card-header">
          <span class="import-badge import-badge-live">正在导入</span>
          <span class="import-progress-number">{Math.round(normalizedPercent)}%</span>
        </div>

        <h2 class="import-title">{stageLabel}</h2>
        <p class="import-description">{progressSummary}</p>

        <div class="import-progress-track" aria-hidden="true">
          <div class="import-progress-fill" style={`width: ${normalizedPercent}%`}></div>
        </div>

        <div class="import-steps">
          {#each IMPORT_STAGES as item, index}
            <div class="import-step" data-state={getStageState(index)}>
              <span class="import-step-dot"></span>
              <span>{item.short}</span>
            </div>
          {/each}
        </div>

        <p class="import-note import-note-progress">
          {#if progress.stage === "done"}
            双语字幕已经生成完成，可以前往资源列表继续查看。
          {:else}
            {activeStageMeta.description}
          {/if}
        </p>

        {#if canCancel}
          <div class="import-progress-actions">
            <button class="btn btn-ghost btn-sm import-cancel-btn" type="button" onclick={onCancel} disabled={isCancellingAsr}>
              {#if isCancellingAsr}正在取消…{:else}取消识别{/if}
            </button>
          </div>
        {/if}
      </article>
    {:else}
      <article class="import-card import-hero-card">
        <h2 class="import-title">导入素材</h2>
        <p class="import-description">
          支持本地音视频与在线视频链接。导入后会自动完成离线识别和中文字幕生成。
        </p>

        <div class="import-actions">
          <button class="btn btn-primary btn-lg" type="button" onclick={() => { void handleLocalImport(); }}>
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.1" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
              <path d="M12 3v12" />
              <path d="m7 10 5 5 5-5" />
              <path d="M5 21h14" />
            </svg>
            选择本地文件
          </button>
          <button class="btn btn-outline btn-lg" type="button" onclick={openOnlineDialog}>
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.1" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
              <path d="M5 12h14" />
              <path d="m12 5 7 7-7 7" />
              <path d="M5 5h.01" />
              <path d="M5 19h.01" />
            </svg>
            导入在线视频
          </button>
        </div>

        <div class="import-showcase" aria-hidden="true">
          <div class="import-showcase-halo import-showcase-halo-a"></div>
          <div class="import-showcase-halo import-showcase-halo-b"></div>
          <div class="import-showcase-orbit import-showcase-orbit-a"></div>
          <div class="import-showcase-orbit import-showcase-orbit-b"></div>

          <div class="import-showcase-float import-showcase-float-left">
            <span class="import-showcase-float-label">Subtitle</span>
            <span class="import-showcase-float-value">双语</span>
          </div>

          <div class="import-showcase-float import-showcase-float-right">
            <span class="import-showcase-float-label">Audio</span>
            <span class="import-showcase-float-value">Wave</span>
          </div>

          <div class="import-showcase-stack">
            <div class="import-showcase-card import-showcase-card-back"></div>
            <div class="import-showcase-card import-showcase-card-middle"></div>
            <div class="import-showcase-card import-showcase-card-front">
              <span class="import-showcase-chip">Import</span>
              <span class="import-showcase-line import-showcase-line-strong"></span>
              <span class="import-showcase-line"></span>
              <span class="import-showcase-line import-showcase-line-short"></span>
              <div class="import-showcase-bars">
                <span></span>
                <span></span>
                <span></span>
                <span></span>
              </div>
            </div>
          </div>
        </div>

        <p class="import-note">
          支持 MP4、MOV、MKV、MP3、WAV、M4A 等常见格式。长视频建议优先本地导入，公开链接适合直接拉取。
        </p>
      </article>
    {/if}
  </div>
</section>

{#if showToast}
  <div class="import-toast" role="status">
    <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
      <polyline points="20 6 9 17 4 12" />
    </svg>
    <span>全部完成，双语字幕已生成</span>
  </div>
{/if}

{#if showOnlineDialog}
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <div class="import-dialog-backdrop" role="presentation" onclick={handleCloseOnlineDialog}></div>
  <div class="import-dialog import-online-dialog" role="dialog" aria-modal="true" aria-labelledby="online-dialog-title">
    <h3 id="online-dialog-title" class="import-dialog-title">导入在线视频</h3>
    <p class="import-dialog-tip">
      部分站点、私有链接或带校验参数的 URL 可能不支持。如果下载失败，建议先手动保存到本地后再导入。
    </p>

    <label class="online-input-block">
      <input
        type="url"
        placeholder="请输入视频网址"
        bind:value={onlineUrl}
        disabled={isSubmittingOnline}
        onkeydown={handleOnlineInputKeydown}
      />
    </label>

    {#if onlineUrlError}
      <div class="online-input-error">{onlineUrlError}</div>
    {/if}

    <div class="import-dialog-actions">
      <button class="btn btn-ghost btn-sm" type="button" onclick={handleCloseOnlineDialog} disabled={isSubmittingOnline}>取消</button>
      <button class="btn btn-primary btn-sm" type="button" onclick={() => { void handleOnlineImportSubmit(); }} disabled={isSubmittingOnline}>
        开始导入
      </button>
    </div>
  </div>
{/if}

{#if showDialog}
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <div class="import-dialog-backdrop" role="presentation" onclick={handleDialogClose}></div>
  <div class="import-dialog" role="dialog" aria-modal="true" aria-labelledby="success-dialog-title">
    <div class="import-dialog-check">
      <svg width="26" height="26" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
        <polyline points="20 6 9 17 4 12" />
      </svg>
    </div>
    <h3 id="success-dialog-title" class="import-dialog-title">导入成功</h3>
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
  .import-page {
    display: flex !important;
    flex-direction: column;
    flex: 1;
    min-height: calc(100vh - 96px);
  }

  .import-shell {
    width: 100%;
    margin: 0;
    flex: 1;
    display: flex;
    min-height: 100%;
    align-items: stretch;
    justify-content: stretch;
  }

  .import-card {
    width: 100%;
    flex: 1;
    min-height: 100%;
    padding: 42px 38px;
    position: relative;
    overflow: hidden;
    text-align: center;
  }

  .import-card::before {
    content: "";
    position: absolute;
    inset: 0;
    pointer-events: none;
    border-radius: inherit;
  }

  .import-card > * {
    position: relative;
    z-index: 1;
  }

  .import-hero-card {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 20px;
  }

  .import-progress-card {
    display: flex;
    flex-direction: column;
    gap: 18px;
  }

  .import-card-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }

  .import-badge {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    padding: 6px 12px;
    border-radius: var(--radius-pill);
    background: rgba(var(--accent-rgb), 0.12);
    border: 1px solid rgba(var(--accent-rgb), 0.18);
    color: var(--accent);
    font-size: var(--font-2xs);
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }

  .import-badge-live::before {
    content: "";
    width: 8px;
    height: 8px;
    border-radius: 999px;
    background: currentColor;
    margin-right: 8px;
    box-shadow: 0 0 0 6px rgba(var(--accent-rgb), 0.12);
  }

  .import-title {
    margin: 0;
    font-size: clamp(2rem, 4vw, 3rem);
    line-height: 1.08;
    letter-spacing: -0.05em;
    color: var(--text-primary);
  }

  .import-description {
    margin: 0 auto;
    max-width: 34rem;
    font-size: 1rem;
    line-height: 1.8;
    color: var(--text-secondary);
  }

  .import-actions {
    display: flex;
    justify-content: center;
    flex-wrap: wrap;
    gap: 12px;
    margin-top: 2px;
  }

  .import-showcase {
    position: relative;
    width: min(760px, 100%);
    height: 300px;
    margin-top: 60px;
    perspective: 1400px;
    transform-style: preserve-3d;
  }

  .import-showcase-halo,
  .import-showcase-orbit {
    position: absolute;
    inset: 50% auto auto 50%;
    transform: translate(-50%, -50%);
    pointer-events: none;
  }

  .import-showcase-halo {
    border-radius: 999px;
    filter: blur(18px);
    opacity: 0.75;
  }

  .import-showcase-halo-a {
    width: 320px;
    height: 320px;
    background: radial-gradient(circle, rgba(var(--accent-rgb), 0.24) 0%, transparent 68%);
    animation: import-halo-drift 8s ease-in-out infinite;
  }

  .import-showcase-halo-b {
    width: 220px;
    height: 220px;
    background: radial-gradient(circle, rgba(129, 140, 248, 0.14) 0%, transparent 72%);
    transform: translate(-30%, -45%);
    animation: import-halo-drift 9s ease-in-out infinite reverse;
  }

  .import-showcase-orbit {
    border-radius: 999px;
    border: 1px solid rgba(var(--accent-rgb), 0.12);
  }

  .import-showcase-orbit-a {
    width: 360px;
    height: 170px;
    transform: translate(-50%, -50%) rotate(-10deg);
    animation: import-orbit-spin 16s linear infinite;
  }

  .import-showcase-orbit-b {
    width: 280px;
    height: 126px;
    border-color: rgba(255, 255, 255, 0.08);
    transform: translate(-50%, -50%) rotate(12deg);
    animation: import-orbit-spin 12s linear infinite reverse;
  }

  .import-showcase-stack {
    position: absolute;
    inset: 50% auto auto 50%;
    width: 320px;
    height: 216px;
    transform: translate(-50%, -44%) rotateX(58deg) rotateZ(-18deg);
    transform-style: preserve-3d;
    animation: import-stage-float 7s ease-in-out infinite;
  }

  .import-showcase-card {
    position: absolute;
    inset: 0;
    overflow: hidden;
    border-radius: 28px;
    border: 1px solid rgba(255, 255, 255, 0.08);
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.34);
  }

  .import-showcase-card-back {
    background: linear-gradient(180deg, rgba(83, 95, 255, 0.12), rgba(14, 18, 28, 0.82));
    transform: translate3d(-18px, -16px, -60px);
    opacity: 0.6;
  }

  .import-showcase-card-middle {
    background: linear-gradient(180deg, rgba(var(--accent-rgb), 0.1), rgba(18, 22, 32, 0.9));
    transform: translate3d(-6px, -6px, -24px);
    opacity: 0.82;
  }

  .import-showcase-card-front {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 22px;
    background:
      linear-gradient(180deg, rgba(255, 255, 255, 0.045), transparent 36%),
      linear-gradient(180deg, rgba(23, 27, 39, 0.96), rgba(11, 14, 21, 0.96));
  }

  .import-showcase-card-front::before {
    content: "";
    position: absolute;
    inset: -18% -34%;
    background:
      linear-gradient(
        112deg,
        transparent 36%,
        rgba(255, 255, 255, 0.02) 43%,
        rgba(255, 255, 255, 0.22) 50%,
        rgba(255, 255, 255, 0.06) 57%,
        transparent 66%
      );
    transform: translateX(-130%) skewX(-18deg);
    mix-blend-mode: screen;
    pointer-events: none;
    animation: import-card-sweep 6.8s ease-in-out infinite;
  }

  .import-showcase-card-front > * {
    position: relative;
    z-index: 1;
  }

  .import-showcase-chip {
    display: inline-flex;
    align-items: center;
    width: fit-content;
    padding: 6px 10px;
    border-radius: 999px;
    background: rgba(var(--accent-rgb), 0.14);
    border: 1px solid rgba(var(--accent-rgb), 0.18);
    color: var(--accent);
    font-size: var(--font-2xs);
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }

  .import-showcase-line {
    display: block;
    height: 10px;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.08);
  }

  .import-showcase-line-strong {
    width: 52%;
    height: 14px;
    background: rgba(255, 255, 255, 0.92);
  }

  .import-showcase-line-short {
    width: 66%;
  }

  .import-showcase-bars {
    margin-top: auto;
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    align-items: end;
    gap: 10px;
    min-height: 78px;
  }

  .import-showcase-bars span {
    display: block;
    border-radius: 14px 14px 10px 10px;
    background: linear-gradient(180deg, rgba(var(--accent-rgb), 0.86), rgba(var(--accent-rgb), 0.28));
    animation: import-bars-pulse 2.8s ease-in-out infinite;
  }

  .import-showcase-bars span:nth-child(1) {
    height: 46%;
    animation-delay: -0.8s;
  }

  .import-showcase-bars span:nth-child(2) {
    height: 74%;
    animation-delay: -1.4s;
  }

  .import-showcase-bars span:nth-child(3) {
    height: 58%;
    animation-delay: -0.4s;
  }

  .import-showcase-bars span:nth-child(4) {
    height: 86%;
    animation-delay: -1.9s;
  }

  .import-showcase-float {
    position: absolute;
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 110px;
    padding: 14px 16px;
    border-radius: 20px;
    border: 1px solid rgba(255, 255, 255, 0.08);
    background: rgba(17, 20, 30, 0.72);
    backdrop-filter: blur(18px);
    -webkit-backdrop-filter: blur(18px);
    box-shadow: 0 18px 42px rgba(0, 0, 0, 0.22);
  }

  .import-showcase-float-left {
    left: 12%;
    top: 26%;
    animation: import-float-card 6.5s ease-in-out infinite;
  }

  .import-showcase-float-right {
    right: 12%;
    bottom: 22%;
    animation: import-float-card 7.2s ease-in-out infinite reverse;
  }

  .import-showcase-float-label {
    font-size: var(--font-2xs);
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-dim);
  }

  .import-showcase-float-value {
    font-size: var(--font-md);
    font-weight: 700;
    color: var(--text-primary);
    letter-spacing: -0.02em;
  }

  .import-note {
    margin: 0 auto;
    max-width: 38rem;
    font-size: var(--font-sm);
    line-height: 1.75;
    color: var(--text-dim);
  }

  .import-note-progress {
    max-width: none;
    text-align: left;
  }

  .import-progress-number {
    font-size: clamp(2rem, 4vw, 3rem);
    line-height: 1;
    font-weight: 800;
    letter-spacing: -0.05em;
    color: var(--text-primary);
    font-variant-numeric: tabular-nums;
  }

  .import-progress-track {
    height: 10px;
    border-radius: 999px;
    overflow: hidden;
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid var(--border-subtle);
  }

  .import-progress-fill {
    height: 100%;
    border-radius: inherit;
    background-image: linear-gradient(90deg, var(--accent) 0%, var(--accent-hover) 48%, var(--accent) 100%);
    background-size: 200% 100%;
    animation: import-shimmer 2.4s linear infinite;
    box-shadow: 0 0 18px rgba(var(--accent-rgb), 0.24);
  }

  .import-steps {
    display: flex;
    flex-wrap: wrap;
    justify-content: center;
    gap: 10px;
  }

  .import-step {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    padding: 7px 12px;
    border-radius: var(--radius-pill);
    background: var(--bg-inset);
    border: 1px solid var(--border-subtle);
    font-size: var(--font-xs);
    color: var(--text-dim);
  }

  .import-step-dot {
    width: 7px;
    height: 7px;
    border-radius: 999px;
    background: currentColor;
    opacity: 0.45;
  }

  .import-step[data-state="done"] {
    color: var(--success);
    background: rgba(52, 211, 153, 0.08);
    border-color: rgba(52, 211, 153, 0.18);
  }

  .import-step[data-state="active"] {
    color: var(--accent);
    background: rgba(var(--accent-rgb), 0.1);
    border-color: rgba(var(--accent-rgb), 0.18);
  }

  .import-progress-actions {
    display: flex;
    justify-content: flex-end;
  }

  .import-cancel-btn {
    min-width: 120px;
  }

  .import-error-bar {
    position: sticky;
    top: 0;
    z-index: var(--z-sticky);
    width: 100%;
    margin: 0 0 12px;
    display: flex;
    align-items: flex-start;
    gap: 10px;
    padding: 14px 16px;
    background: var(--danger-soft);
    border: 1px solid var(--danger-border);
    border-radius: var(--radius-lg);
    color: var(--danger);
    font-size: var(--font-sm);
    line-height: 1.55;
    backdrop-filter: blur(12px);
    -webkit-backdrop-filter: blur(12px);
  }

  .import-error-copy {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .import-error-msg {
    min-width: 0;
    word-break: break-word;
  }

  .import-error-hint {
    padding: 10px 12px;
    border-radius: var(--radius-md);
    background: rgba(248, 113, 113, 0.08);
    border: 1px solid var(--danger-border);
    color: var(--danger);
    font-size: var(--font-xs);
    line-height: 1.55;
  }

  .import-error-close {
    width: 30px;
    height: 30px;
    display: grid;
    place-items: center;
    border: none;
    border-radius: 999px;
    background: transparent;
    color: currentColor;
    cursor: pointer;
    flex-shrink: 0;
    opacity: 0.72;
    transition: opacity var(--transition-fast), background var(--transition-fast);
  }

  .import-error-close:hover {
    opacity: 1;
    background: rgba(248, 113, 113, 0.08);
  }

  .import-toast {
    position: fixed;
    left: calc(var(--sidebar-w, 220px) + (100vw - var(--sidebar-w, 220px)) / 2);
    bottom: 24px;
    transform: translateX(-50%);
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 11px 18px;
    border-radius: var(--radius-pill);
    background: var(--bg-glass);
    border: 1px solid var(--border);
    color: var(--success);
    font-size: var(--font-sm);
    font-weight: 600;
    box-shadow: var(--shadow-md);
    backdrop-filter: blur(16px);
    -webkit-backdrop-filter: blur(16px);
    z-index: var(--z-toast);
  }

  .import-dialog-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.48);
    backdrop-filter: blur(8px);
    -webkit-backdrop-filter: blur(8px);
    z-index: var(--z-overlay);
  }

  .import-dialog {
    position: fixed;
    top: 50%;
    left: calc(var(--sidebar-w, 220px) + (100vw - var(--sidebar-w, 220px)) / 2);
    transform: translate(-50%, -50%);
    width: 360px;
    padding: 30px 28px 22px;
    border-radius: 24px;
    border: 1px solid var(--border);
    background: var(--bg-raised);
    box-shadow: var(--shadow-lg);
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 12px;
    text-align: center;
    z-index: var(--z-modal);
  }

  .import-online-dialog {
    width: 460px;
    align-items: stretch;
    text-align: left;
  }

  .import-dialog-check {
    width: 58px;
    height: 58px;
    display: grid;
    place-items: center;
    border-radius: 999px;
    background: rgba(52, 211, 153, 0.12);
    border: 1px solid rgba(52, 211, 153, 0.18);
    color: var(--success);
  }

  .import-dialog-title {
    font-size: var(--font-xl);
    letter-spacing: -0.02em;
    color: var(--text-primary);
  }

  .import-dialog-desc {
    max-width: 320px;
    font-size: var(--font-sm);
    line-height: 1.7;
    color: var(--text-secondary);
  }

  .import-dialog-suppress {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: var(--font-xs);
    color: var(--text-dim);
    cursor: pointer;
    user-select: none;
  }

  .import-dialog-suppress input {
    cursor: pointer;
    accent-color: var(--accent);
  }

  .import-dialog-actions {
    width: 100%;
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 4px;
  }

  .online-input-block {
    display: flex;
    flex-direction: column;
    gap: 8px;
    margin-top: 4px;
    font-size: var(--font-xs);
    color: var(--text-secondary);
  }

  .online-input-block input {
    width: 100%;
    padding: 12px 14px;
    border-radius: 16px;
    border: 1px solid var(--border);
    background: var(--bg-inset);
    color: var(--text-primary);
    font: inherit;
    transition: border-color var(--transition-fast), background var(--transition-fast), box-shadow var(--transition-fast);
  }

  .online-input-block input:focus-visible {
    background: var(--bg-surface);
  }

  .online-input-error {
    font-size: var(--font-xs);
    color: var(--danger);
    margin-top: -2px;
  }

  .import-dialog-tip {
    font-size: var(--font-xs);  
  }

  @keyframes import-shimmer {
    0% {
      background-position: 200% 0;
    }
    100% {
      background-position: -200% 0;
    }
  }

  @keyframes import-stage-float {
    0%, 100% {
      transform: translate(-50%, -44%) rotateX(58deg) rotateZ(-18deg) translateY(0);
    }
    50% {
      transform: translate(-50%, -46%) rotateX(58deg) rotateZ(-18deg) translateY(-8px);
    }
  }

  @keyframes import-orbit-spin {
    0% {
      transform: translate(-50%, -50%) rotate(0deg);
    }
    100% {
      transform: translate(-50%, -50%) rotate(360deg);
    }
  }

  @keyframes import-halo-drift {
    0%, 100% {
      transform: translate(-50%, -50%) scale(1);
    }
    50% {
      transform: translate(-46%, -54%) scale(1.08);
    }
  }

  @keyframes import-float-card {
    0%, 100% {
      transform: translateY(0);
    }
    50% {
      transform: translateY(-10px);
    }
  }

  @keyframes import-bars-pulse {
    0%, 100% {
      filter: brightness(0.95);
      transform: scaleY(0.96);
    }
    50% {
      filter: brightness(1.08);
      transform: scaleY(1.02);
    }
  }

  @keyframes import-card-sweep {
    0%, 12% {
      transform: translateX(-130%) skewX(-18deg);
      opacity: 0;
    }
    24% {
      opacity: 0.9;
    }
    46% {
      transform: translateX(118%) skewX(-18deg);
      opacity: 0;
    }
    100% {
      transform: translateX(118%) skewX(-18deg);
      opacity: 0;
    }
  }

  @media (max-width: 900px) {
    .import-page {
      min-height: calc(100vh - 80px);
    }

    .import-shell,
    .import-error-bar {
      width: 100%;
    }

    .import-toast,
    .import-dialog,
    .import-online-dialog {
      left: 50%;
    }
  }

  @media (max-width: 640px) {
    .import-page {
      min-height: calc(100vh - 72px);
    }

    .import-card {
      padding: 28px 22px;
      border-radius: 22px;
    }

    .import-card-header,
    .import-actions,
    .import-dialog-actions {
      flex-direction: column;
      align-items: stretch;
    }

    .import-progress-actions {
      justify-content: stretch;
    }

    .import-showcase {
      height: 236px;
    }

    .import-showcase-stack {
      width: 250px;
      height: 174px;
      transform: translate(-50%, -45%) rotateX(58deg) rotateZ(-18deg);
    }

    .import-showcase-float-left {
      left: 2%;
      top: 18%;
    }

    .import-showcase-float-right {
      right: 2%;
      bottom: 18%;
    }

    .import-cancel-btn {
      width: 100%;
    }

    .import-dialog,
    .import-online-dialog {
      width: min(460px, calc(100vw - 24px));
      padding: 24px 20px 18px;
    }
  }
</style>
