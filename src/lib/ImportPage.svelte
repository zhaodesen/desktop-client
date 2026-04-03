<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import type { ImportProgress, ImportProgressStage } from "../shared/types";

  interface Props {
    progress: ImportProgress;
    importError: string | undefined;
    importSuccessName: string | undefined;
    importSuccessKind: "bilingual" | "original" | "translation-failed";
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
    importSuccessKind,
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
  const SHOWCASE_BAR_BASES = [0.38, 0.62, 0.5, 0.78];

  let suppressDialog = $state(false);
  let dialogSuppressChecked = $state(false);
  let showToast = $state(false);
  let showOnlineDialog = $state(false);
  let onlineUrl = $state("");
  let onlineUrlError = $state<string | undefined>(undefined);
  let isSubmittingOnline = $state(false);
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
  let successSummary = $derived.by(() => {
    if (importSuccessKind === "translation-failed") {
      return "素材已导入，原文字幕可用，但中文字幕生成失败。";
    }
    if (importSuccessKind === "original") {
      return "素材已导入，已生成原文字幕。";
    }
    return "素材已导入，双语字幕已生成完毕。";
  });
  let importErrorHint = $derived(getImportErrorHint(importError));
  let progressRatio = $derived(normalizedPercent / 100);
  let showcaseGlow = $derived(progress.active ? 0.16 + (progressRatio * 0.28) : 0.12);
  let showcaseBars = $derived.by(() =>
    SHOWCASE_BAR_BASES.map((base, index) => {
      const wave = Math.sin((progressRatio * Math.PI * 1.2) + (index * 0.82)) * 0.08;
      const height = Math.max(
        0.26,
        Math.min(0.96, base + (progress.active ? (progressRatio * 0.18) : 0) + wave),
      );
      return `${Math.round(height * 100)}%`;
    }),
  );
  let showcaseStyle = $derived(
    [
      `--import-progress: ${progressRatio.toFixed(4)}`,
      `--showcase-glow: ${showcaseGlow.toFixed(4)}`,
    ].join("; "),
  );

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
    try {
      await onImportMedia();
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
  <article class="import-hero" data-mode={progress.active ? "active" : "idle"} style={showcaseStyle}>
    <div class="import-bg" aria-hidden="true">
      <div class="import-orb import-orb-a"></div>
      <div class="import-orb import-orb-b"></div>
    </div>

    <div class="import-idle">
      <h2 class="import-title">导入素材</h2>
      <p class="import-desc">
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

      <p class="import-note">
        支持 MP4、MOV、MKV、MP3、WAV、M4A 等常见格式。长视频建议优先本地导入，公开链接适合直接拉取。
      </p>
    </div>

    <div class="import-card">
      <div class="import-card-inner">
        <div class="import-card-shine" aria-hidden="true"></div>

        <div class="import-face-decor" aria-hidden="true">
          <span class="import-chip">Import</span>
          <span class="import-skel import-skel-bold"></span>
          <span class="import-skel"></span>
          <span class="import-skel import-skel-short"></span>
          <div class="import-bars">
            {#each showcaseBars as h}
              <span style={`height: ${h}`}></span>
            {/each}
          </div>
        </div>

        <div class="import-face-progress" role="status" aria-live="polite">
          <div class="import-fp-head">
            <span class="import-badge import-badge-live">正在导入</span>
            <span class="import-pct">{Math.round(normalizedPercent)}%</span>
          </div>
          <div class="import-fp-body">
            <h3 class="import-stage-label">{stageLabel}</h3>
            <p class="import-stage-desc">{progressSummary}</p>
          </div>
          <div class="import-track">
            <div class="import-track-fill" style={`width: ${normalizedPercent}%`}></div>
          </div>
          <div class="import-steps">
            {#each IMPORT_STAGES as item, index}
              <div class="import-step" data-state={getStageState(index)}>
                <span class="import-step-dot"></span>
                <span>{item.short}</span>
              </div>
            {/each}
          </div>
          <p class="import-stage-note">
            {#if progress.stage === "done"}
              {successSummary} 可以前往资源列表继续查看。
            {:else}
              {activeStageMeta.description}
            {/if}
          </p>
          {#if canCancel}
            <div class="import-cancel-wrap">
              <button class="btn btn-ghost btn-sm import-cancel-btn" type="button" onclick={onCancel} disabled={isCancellingAsr}>
                {#if isCancellingAsr}正在取消…{:else}取消识别{/if}
              </button>
            </div>
          {/if}
        </div>
      </div>
    </div>
  </article>
</section>

{#if showToast}
  <div class="import-toast" role="status">
    <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
      <polyline points="20 6 9 17 4 12" />
    </svg>
    <span>
      {#if importSuccessKind === "translation-failed"}
        全部完成，原文字幕已生成，中文字幕生成失败
      {:else if importSuccessKind === "original"}
        全部完成，原文字幕已生成
      {:else}
        全部完成，双语字幕已生成
      {/if}
    </span>
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
        「{importSuccessName}」已导入，
        {#if importSuccessKind === "translation-failed"}
          原文字幕可用，但中文字幕生成失败。
        {:else if importSuccessKind === "original"}
          已生成原文字幕。
        {:else}
          双语字幕已生成完毕。
        {/if}
      {:else}
        {successSummary}
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
  /* ═══════════════════════════════════════════
     Layout
     ═══════════════════════════════════════════ */
  .import-page {
    display: flex !important;
    flex-direction: column;
    flex: 1;
    min-height: calc(100vh - 96px);
  }

  .import-hero {
    --import-progress: 0;
    --showcase-glow: 0.12;
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 0;
    position: relative;
    overflow: hidden;
    padding: 42px 38px;
    text-align: center;
  }

  .import-hero > * {
    position: relative;
    z-index: 1;
  }

  /* ═══════════════════════════════════════════
     Ambient background orbs
     ═══════════════════════════════════════════ */
  .import-bg {
    position: absolute !important;
    inset: 0;
    z-index: 0 !important;
    pointer-events: none;
    overflow: hidden;
  }

  .import-orb {
    position: absolute;
    border-radius: 50%;
    will-change: transform, opacity;
  }

  .import-orb-a {
    width: 420px;
    height: 420px;
    top: 12%;
    left: 28%;
    background: radial-gradient(circle, rgba(var(--accent-rgb), 0.18) 0%, transparent 68%);
    filter: blur(80px);
    opacity: 0.6;
    animation: import-orb-a 14s ease-in-out infinite;
    transition:
      opacity 900ms cubic-bezier(0.4, 0, 0.2, 1) 200ms,
      filter 900ms cubic-bezier(0.4, 0, 0.2, 1) 200ms,
      transform 900ms cubic-bezier(0.16, 1, 0.3, 1) 200ms;
  }

  .import-orb-b {
    width: 320px;
    height: 320px;
    bottom: 14%;
    right: 24%;
    background: radial-gradient(circle, rgba(129, 140, 248, 0.14) 0%, transparent 70%);
    filter: blur(70px);
    opacity: 0.45;
    animation: import-orb-b 16s ease-in-out infinite;
    transition:
      opacity 900ms cubic-bezier(0.4, 0, 0.2, 1) 200ms,
      filter 900ms cubic-bezier(0.4, 0, 0.2, 1) 200ms,
      transform 900ms cubic-bezier(0.16, 1, 0.3, 1) 200ms;
  }

  .import-hero[data-mode="active"] .import-orb-a {
    opacity: calc(0.55 + var(--showcase-glow));
    filter: blur(100px);
    transform: scale(calc(1.3 + var(--import-progress) * 0.3));
  }

  .import-hero[data-mode="active"] .import-orb-b {
    opacity: calc(0.35 + var(--showcase-glow) * 0.6);
    filter: blur(90px);
    transform: scale(calc(1.2 + var(--import-progress) * 0.2));
  }

  /* ═══════════════════════════════════════════
     Step 1 — Idle copy block (first out, last in)
     ═══════════════════════════════════════════ */
  .import-idle {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 20px;
    max-width: 42rem;
    /* REVERSE: reappear last */
    transition:
      opacity 360ms cubic-bezier(0.4, 0, 0.2, 1) 380ms,
      transform 400ms cubic-bezier(0.16, 1, 0.3, 1) 380ms,
      filter 400ms cubic-bezier(0.4, 0, 0.2, 1) 380ms,
      max-height 450ms cubic-bezier(0.16, 1, 0.3, 1) 320ms,
      margin 450ms cubic-bezier(0.16, 1, 0.3, 1) 320ms;
  }

  .import-hero[data-mode="active"] .import-idle {
    opacity: 0;
    transform: translateY(-14px) scale(0.97);
    filter: blur(6px);
    pointer-events: none;
    max-height: 0;
    margin: 0;
    overflow: hidden;
    /* FORWARD: disappear first (0ms) */
    transition:
      opacity 280ms cubic-bezier(0.4, 0, 0.2, 1) 0ms,
      transform 280ms cubic-bezier(0.16, 1, 0.3, 1) 0ms,
      filter 280ms cubic-bezier(0.4, 0, 0.2, 1) 0ms,
      max-height 400ms cubic-bezier(0.16, 1, 0.3, 1) 30ms,
      margin 400ms cubic-bezier(0.16, 1, 0.3, 1) 30ms;
  }

  /* ═══════════════════════════════════════════
     Step 2 — Card grows to 50% and centers
     ═══════════════════════════════════════════ */
  .import-card {
    margin-top: 36px;
    animation: import-card-float 7s ease-in-out infinite;
    /* REVERSE: margin returns */
    transition: margin-top 420ms cubic-bezier(0.32, 0.72, 0, 1) 220ms;
  }

  .import-hero[data-mode="active"] .import-card {
    margin-top: 0;
    animation: none;
    /* FORWARD: margin→0 after copy starts collapsing */
    transition: margin-top 420ms cubic-bezier(0.32, 0.72, 0, 1) 160ms;
  }

  .import-card-inner {
    position: relative;
    width: 340px;
    height: 220px;
    border-radius: 24px;
    overflow: hidden;
    background:
      linear-gradient(180deg, rgba(255, 255, 255, 0.045), transparent 40%),
      linear-gradient(180deg, rgba(23, 27, 39, 0.97), rgba(11, 14, 21, 0.97));
    border: 1px solid rgba(255, 255, 255, 0.08);
    box-shadow:
      0 20px 60px rgba(0, 0, 0, 0.28),
      0 0 0 1px rgba(255, 255, 255, 0.03);
    transform: translateZ(0);
    -webkit-transform: translateZ(0);
    backface-visibility: hidden;
    -webkit-backface-visibility: hidden;
    will-change: width, height;
    /* REVERSE: shrink back */
    transition:
      width 440ms cubic-bezier(0.32, 0.72, 0, 1) 220ms,
      height 440ms cubic-bezier(0.32, 0.72, 0, 1) 220ms,
      border-radius 440ms cubic-bezier(0.32, 0.72, 0, 1) 220ms,
      box-shadow 500ms cubic-bezier(0.4, 0, 0.2, 1) 220ms;
  }

  .import-hero[data-mode="active"] .import-card-inner {
    width: min(580px, calc((100vw - var(--sidebar-w, 220px)) * 0.52));
    height: min(450px, 50vh);
    border-radius: 28px;
    box-shadow:
      0 32px 80px rgba(0, 0, 0, 0.36),
      0 0 0 1px rgba(255, 255, 255, 0.06),
      0 0 80px rgba(var(--accent-rgb), calc(0.04 + var(--import-progress) * 0.12));
    /* FORWARD: grow after copy starts fading */
    transition:
      width 440ms cubic-bezier(0.32, 0.72, 0, 1) 160ms,
      height 440ms cubic-bezier(0.32, 0.72, 0, 1) 160ms,
      border-radius 440ms cubic-bezier(0.32, 0.72, 0, 1) 160ms,
      box-shadow 500ms cubic-bezier(0.4, 0, 0.2, 1) 160ms;
  }

  /* Card light sweep */
  .import-card-shine {
    position: absolute;
    inset: -18% -34%;
    z-index: 3;
    background:
      linear-gradient(
        112deg,
        transparent 36%,
        rgba(255, 255, 255, 0.02) 43%,
        rgba(255, 255, 255, 0.18) 50%,
        rgba(255, 255, 255, 0.04) 57%,
        transparent 66%
      );
    transform: translateX(-130%) skewX(-18deg);
    mix-blend-mode: screen;
    pointer-events: none;
    animation: import-sweep 7s ease-in-out infinite;
  }

  /* ═══════════════════════════════════════════
     Decorative face — fades with copy block
     ═══════════════════════════════════════════ */
  .import-face-decor {
    position: absolute;
    inset: 0;
    z-index: 1;
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 22px;
    /* REVERSE: reappear with card shrink */
    transition:
      opacity 300ms cubic-bezier(0.4, 0, 0.2, 1) 280ms,
      filter 300ms cubic-bezier(0.4, 0, 0.2, 1) 280ms;
  }

  .import-hero[data-mode="active"] .import-face-decor {
    opacity: 0;
    filter: blur(8px);
    pointer-events: none;
    /* FORWARD: fade out immediately with copy */
    transition:
      opacity 240ms cubic-bezier(0.4, 0, 0.2, 1) 0ms,
      filter 240ms cubic-bezier(0.4, 0, 0.2, 1) 0ms;
  }

  .import-chip {
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

  .import-skel {
    display: block;
    height: 10px;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.08);
  }

  .import-skel-bold {
    width: 52%;
    height: 14px;
    background: rgba(255, 255, 255, 0.88);
  }

  .import-skel-short {
    width: 66%;
  }

  .import-bars {
    margin-top: auto;
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    align-items: end;
    gap: 10px;
    min-height: 72px;
  }

  .import-bars span {
    display: block;
    border-radius: 14px 14px 10px 10px;
    background: linear-gradient(180deg, rgba(var(--accent-rgb), 0.86), rgba(var(--accent-rgb), 0.28));
    animation: import-bars-pulse 2.8s ease-in-out infinite;
    transition: height 420ms cubic-bezier(0.22, 1, 0.36, 1);
  }

  .import-bars span:nth-child(1) { height: 46%; animation-delay: -0.8s; }
  .import-bars span:nth-child(2) { height: 74%; animation-delay: -1.4s; }
  .import-bars span:nth-child(3) { height: 58%; animation-delay: -0.4s; }
  .import-bars span:nth-child(4) { height: 86%; animation-delay: -1.9s; }

  /* ═══════════════════════════════════════════
     Step 3 — Progress face (last in, first out)
     ═══════════════════════════════════════════ */
  .import-face-progress {
    position: absolute;
    inset: 0;
    z-index: 2;
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding: 28px 30px;
    text-align: left;
    opacity: 0;
    transform: translateY(10px);
    filter: blur(6px);
    pointer-events: none;
    /* REVERSE: disappear first (0ms) */
    transition:
      opacity 280ms cubic-bezier(0.4, 0, 0.2, 1) 0ms,
      transform 280ms cubic-bezier(0.16, 1, 0.3, 1) 0ms,
      filter 280ms cubic-bezier(0.4, 0, 0.2, 1) 0ms;
  }

  .import-hero[data-mode="active"] .import-face-progress {
    opacity: 1;
    transform: translateY(0);
    filter: blur(0);
    pointer-events: auto;
    /* FORWARD: appear LAST — after card finishes growing (~400ms) */
    transition:
      opacity 400ms cubic-bezier(0.4, 0, 0.2, 1) 420ms,
      transform 400ms cubic-bezier(0.16, 1, 0.3, 1) 420ms,
      filter 400ms cubic-bezier(0.4, 0, 0.2, 1) 420ms;
  }

  .import-fp-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 14px;
  }

  .import-fp-body {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .import-pct {
    font-size: clamp(2rem, 4vw, 2.8rem);
    line-height: 1;
    font-weight: 800;
    letter-spacing: -0.04em;
    color: var(--text-primary);
    font-variant-numeric: tabular-nums;
  }

  .import-stage-label {
    margin: 0;
    font-size: clamp(1.25rem, 2.5vw, 1.6rem);
    font-weight: 700;
    letter-spacing: -0.02em;
    color: var(--text-primary);
    line-height: 1.2;
  }

  .import-stage-desc {
    margin: 0;
    font-size: var(--font-sm);
    line-height: 1.65;
    color: var(--text-secondary);
  }

  .import-stage-note {
    margin: 0;
    font-size: var(--font-xs);
    line-height: 1.7;
    color: var(--text-dim);
  }

  .import-cancel-wrap {
    display: flex;
    justify-content: flex-end;
    margin-top: auto;
  }

  .import-cancel-btn {
    min-width: 120px;
  }

  /* ═══════════════════════════════════════════
     Shared typography
     ═══════════════════════════════════════════ */
  .import-title {
    margin: 0;
    font-size: clamp(2rem, 4vw, 3rem);
    line-height: 1.08;
    letter-spacing: -0.05em;
    color: var(--text-primary);
  }

  .import-desc {
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

  .import-note {
    margin: 0 auto;
    max-width: 38rem;
    font-size: var(--font-sm);
    line-height: 1.75;
    color: var(--text-dim);
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
    animation: import-badge-pulse 2s ease-in-out infinite;
  }

  /* ═══════════════════════════════════════════
     Progress track
     ═══════════════════════════════════════════ */
  .import-track {
    height: 8px;
    border-radius: 999px;
    overflow: hidden;
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.04);
  }

  .import-track-fill {
    height: 100%;
    border-radius: inherit;
    background-image: linear-gradient(90deg, var(--accent) 0%, var(--accent-hover) 48%, var(--accent) 100%);
    background-size: 200% 100%;
    animation: import-shimmer 2.4s linear infinite;
    box-shadow: 0 0 14px rgba(var(--accent-rgb), 0.28);
    transition: width 300ms cubic-bezier(0.16, 1, 0.3, 1);
  }

  /* ═══════════════════════════════════════════
     Steps
     ═══════════════════════════════════════════ */
  .import-steps {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }

  .import-step {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 6px 12px;
    border-radius: var(--radius-pill);
    background: var(--bg-inset);
    border: 1px solid var(--border-subtle);
    font-size: var(--font-xs);
    color: var(--text-dim);
    transition: color 300ms, background 300ms, border-color 300ms;
  }

  .import-step-dot {
    width: 7px;
    height: 7px;
    border-radius: 999px;
    background: currentColor;
    opacity: 0.45;
    transition: opacity 300ms;
  }

  .import-step[data-state="done"] {
    color: var(--success);
    background: rgba(52, 211, 153, 0.08);
    border-color: rgba(52, 211, 153, 0.18);
  }

  .import-step[data-state="done"] .import-step-dot { opacity: 0.8; }

  .import-step[data-state="active"] {
    color: var(--accent);
    background: rgba(var(--accent-rgb), 0.1);
    border-color: rgba(var(--accent-rgb), 0.18);
  }

  .import-step[data-state="active"] .import-step-dot { opacity: 1; }

  /* ═══════════════════════════════════════════
     Error bar
     ═══════════════════════════════════════════ */
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

  .import-error-msg { min-width: 0; word-break: break-word; }

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

  /* ═══════════════════════════════════════════
     Toast
     ═══════════════════════════════════════════ */
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
    animation: import-toast-enter 420ms cubic-bezier(0.16, 1, 0.3, 1) both;
  }

  /* ═══════════════════════════════════════════
     Dialogs
     ═══════════════════════════════════════════ */
  .import-dialog-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.48);
    backdrop-filter: blur(8px);
    -webkit-backdrop-filter: blur(8px);
    z-index: var(--z-overlay);
    animation: import-backdrop-enter 300ms cubic-bezier(0.4, 0, 0.2, 1) both;
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
    animation: import-dialog-enter 400ms cubic-bezier(0.16, 1, 0.3, 1) 60ms both;
  }

  .import-online-dialog { width: 460px; align-items: stretch; text-align: left; }

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

  .import-dialog-title { font-size: var(--font-xl); letter-spacing: -0.02em; color: var(--text-primary); }
  .import-dialog-desc { max-width: 320px; font-size: var(--font-sm); line-height: 1.7; color: var(--text-secondary); }
  .import-dialog-suppress { display: flex; align-items: center; gap: 8px; font-size: var(--font-xs); color: var(--text-dim); cursor: pointer; user-select: none; }
  .import-dialog-suppress input { cursor: pointer; accent-color: var(--accent); }
  .import-dialog-actions { width: 100%; display: flex; justify-content: flex-end; gap: 8px; margin-top: 4px; }
  .import-dialog-tip { font-size: var(--font-xs); }

  .online-input-block { display: flex; flex-direction: column; gap: 8px; margin-top: 4px; font-size: var(--font-xs); color: var(--text-secondary); }
  .online-input-block input { width: 100%; padding: 12px 14px; border-radius: 16px; border: 1px solid var(--border); background: var(--bg-inset); color: var(--text-primary); font: inherit; transition: border-color var(--transition-fast), background var(--transition-fast), box-shadow var(--transition-fast); }
  .online-input-block input:focus-visible { background: var(--bg-surface); }
  .online-input-error { font-size: var(--font-xs); color: var(--danger); margin-top: -2px; }

  /* ═══════════════════════════════════════════
     Keyframes
     ═══════════════════════════════════════════ */
  @keyframes import-card-float {
    0%, 100% { translate: 0 0; }
    50% { translate: 0 -7px; }
  }

  @keyframes import-orb-a {
    0%, 100% { transform: translate(0, 0) scale(1); }
    33% { transform: translate(20px, -15px) scale(1.06); }
    66% { transform: translate(-12px, 10px) scale(0.97); }
  }

  @keyframes import-orb-b {
    0%, 100% { transform: translate(0, 0) scale(1); }
    33% { transform: translate(-18px, 12px) scale(1.04); }
    66% { transform: translate(14px, -10px) scale(0.95); }
  }

  @keyframes import-sweep {
    0%, 12% { transform: translateX(-130%) skewX(-18deg); opacity: 0; }
    24% { opacity: 0.9; }
    46%, 100% { transform: translateX(118%) skewX(-18deg); opacity: 0; }
  }

  @keyframes import-bars-pulse {
    0%, 100% { filter: brightness(0.95); transform: scaleY(0.96); }
    50% { filter: brightness(1.08); transform: scaleY(1.02); }
  }

  @keyframes import-shimmer {
    0% { background-position: 200% 0; }
    100% { background-position: -200% 0; }
  }

  @keyframes import-badge-pulse {
    0%, 100% { opacity: 1; box-shadow: 0 0 0 0 currentColor; }
    50% { opacity: 0.5; box-shadow: 0 0 0 6px transparent; }
  }

  @keyframes import-toast-enter {
    0% { opacity: 0; transform: translateX(-50%) translateY(10px) scale(0.96); }
    100% { opacity: 1; transform: translateX(-50%) translateY(0) scale(1); }
  }

  @keyframes import-backdrop-enter {
    from { opacity: 0; }
    to { opacity: 1; }
  }

  @keyframes import-dialog-enter {
    0% { opacity: 0; transform: translate(-50%, -46%) scale(0.94); }
    100% { opacity: 1; transform: translate(-50%, -50%) scale(1); }
  }

  /* ═══════════════════════════════════════════
     Responsive
     ═══════════════════════════════════════════ */
  @media (max-width: 900px) {
    .import-page { min-height: calc(100vh - 80px); }
    .import-toast, .import-dialog, .import-online-dialog { left: 50%; }

    .import-hero[data-mode="active"] .import-card-inner {
      width: min(520px, calc(100vw - 60px));
      height: min(420px, 52vh);
    }
  }

  @media (max-width: 640px) {
    .import-page { min-height: calc(100vh - 72px); }
    .import-hero { padding: 28px 22px; }

    .import-actions,
    .import-dialog-actions,
    .import-fp-head {
      flex-direction: column;
      align-items: stretch;
    }

    .import-fp-head { align-items: flex-start; }

    .import-card-inner { width: 260px; height: 180px; }

    .import-hero[data-mode="active"] .import-card-inner {
      width: min(460px, calc(100vw - 32px));
      height: min(400px, 55vh);
    }

    .import-face-progress { padding: 20px; gap: 12px; }

    .import-steps { gap: 6px; }
    .import-step { padding: 5px 10px; }
    .import-cancel-btn { width: 100%; }

    .import-dialog, .import-online-dialog {
      width: min(460px, calc(100vw - 24px));
      padding: 24px 20px 18px;
    }
  }

  /* ═══════════════════════════════════════════
     Reduced motion
     ═══════════════════════════════════════════ */
  @media (prefers-reduced-motion: reduce) {
    .import-idle,
    .import-card,
    .import-card-inner,
    .import-face-decor,
    .import-face-progress,
    .import-orb,
    .import-track-fill,
    .import-toast,
    .import-dialog,
    .import-dialog-backdrop,
    .import-badge-live::before,
    .import-bars span,
    .import-step {
      animation: none !important;
      transition-duration: 0ms !important;
    }
  }
</style>
