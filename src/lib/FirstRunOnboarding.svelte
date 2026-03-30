<script lang="ts">
  import { onDestroy } from "svelte";
  import type { ModelInfo, ModelStatus } from "../shared/types";
  import ParticleGlobe from "./ParticleGlobe.svelte";
  import StarField from "./StarField.svelte";

  type OnboardingStep = "select-model" | "downloading" | "ready";

  type ModelGuide = {
    pros: string[];
    cons: string[];
    recommended?: boolean;
  };

  interface Props {
    topInset: number;
    step: OnboardingStep;
    models: ModelInfo[];
    modelsStatusMap: Map<string, ModelStatus>;
    selectedModelId: string | undefined;
    downloadPercent: number;
    downloadMessage: string;
    error: string | undefined;
    modelGuides: Record<string, ModelGuide>;
    onSelectModel: (id: string) => void;
    onRetry: () => void;
    onBack: () => void;
    onSkip: () => void;
    onStart: () => void;
  }

  const {
    topInset,
    step,
    models,
    modelsStatusMap,
    selectedModelId,
    downloadPercent,
    downloadMessage,
    error,
    modelGuides,
    onSelectModel,
    onRetry,
    onBack,
    onSkip,
    onStart,
  }: Props = $props();

  const selectedModel = $derived(models.find((model) => model.id === selectedModelId));
  const isDownloadStage = $derived(step !== "select-model");
  const isDownloadComplete = $derived(downloadPercent >= 100 && !error);
  const START_EXIT_DURATION_MS = 760;
  let isExitingToHome = $state(false);
  let startExitTimer: ReturnType<typeof setTimeout> | undefined;

  const modelHints: Record<string, string> = {
    tiny: "最快最轻，低配优先",
    base: "日常首选，均衡稳定",
    small: "更高准确率",
    medium: "长音频更稳",
    "large-v3-turbo": "最高质量",
  };

  function handleStartClick() {
    if (isExitingToHome) return;
    isExitingToHome = true;
    startExitTimer = setTimeout(() => {
      startExitTimer = undefined;
      onStart();
    }, START_EXIT_DURATION_MS);
  }

  onDestroy(() => {
    if (startExitTimer) {
      clearTimeout(startExitTimer);
    }
  });
</script>

<section
  class="first-run-mask"
  class:is-exiting-home={isExitingToHome}
  style={`--top-inset: ${topInset}px;`}
  aria-label="首次启动引导"
>
  {#if topInset === 0}
    <div class="first-run-drag-region" data-tauri-drag-region></div>
  {/if}
  <div class="first-run-stage">
    <div
      class="first-run-panel first-run-panel-shell"
      class:is-download-stage={isDownloadStage}
      class:is-ready-stage={step === "ready"}
      class:is-exiting-home={isExitingToHome}
    >
      <div class="first-run-viewport">
        <div class="first-run-scene-stack">
          <div class="first-run-scene first-run-scene-select" aria-hidden={isDownloadStage}>
            <div class="first-run-select-layer first-run-panel-select">
              <div class="first-run-head">
                <div class="hero-copy">
                  <h1>开始之前，请先选择一个识别模型</h1>
                  <p>先选一个默认模型。之后仍可在设置里切换或重新下载。</p>
                </div>
                <button class="skip-button" type="button" onclick={onSkip}>跳过</button>
              </div>

              <div class="model-grid">
                {#each models as model (model.id)}
                  {@const guide = modelGuides[model.id]}
                  {@const status = modelsStatusMap.get(model.id)}
                  <button
                    class="model-card"
                    type="button"
                    onclick={() => onSelectModel(model.id)}
                  >
                    <div class="model-card-top">
                      <div class="model-card-title-row">
                        <div class="model-card-title">{model.label}</div>
                        {#if status?.installed}
                          <span class="pill pill-success">已下载</span>
                        {/if}
                      </div>
                    </div>

                    <p class="model-card-hint">{modelHints[model.id] ?? "离线识别模型"}</p>
                    <p class="model-card-desc">{model.description}</p>

                  </button>
                {/each}
              </div>
            </div>
          </div>

          <div class="first-run-scene first-run-scene-download" aria-hidden={!isDownloadStage}>
            <div class="first-run-download-layer">
              <div class="download-background">
                <StarField />
              </div>

              <div class="download-globe-shell">
                <div class="download-canvas-layer">
                  <ParticleGlobe completed={isDownloadComplete} />
                </div>
              </div>

              <div class="download-overlay">
                <div class="download-content">
                  <div class="download-info">
                    <h2 class="download-title">{selectedModel?.label ?? "识别模型"}</h2>
                    <p class="download-msg">{downloadMessage}</p>
                  </div>

                  {#if error}
                    <div class="download-error">
                      <p>{error}</p>
                      <div class="download-actions">
                        <button class="btn btn-ghost" type="button" onclick={onBack}>返回重选</button>
                        <button class="btn btn-primary" type="button" onclick={onRetry}>重新下载</button>
                      </div>
                    </div>
                  {/if}

                  <div class="download-cta">
                    <div class="cta-text">
                      <h2 class="cta-title">{selectedModel?.label ?? "模型"} 已就绪</h2>
                      <p class="cta-desc">现在可以开始导入音频或视频生成字幕了</p>
                    </div>
                    <button class="btn btn-primary cta-btn" type="button" disabled={isExitingToHome} onclick={handleStartClick}>开始使用</button>
                  </div>
                </div>

                <div class="progress-footer">
                  <div class="progress-meta">
                    <span>下载进度</span>
                    <strong>{Math.round(downloadPercent)}%</strong>
                  </div>
                  <div class="progress-track">
                    <div class="progress-fill" style={`width: ${Math.max(0, Math.min(100, downloadPercent))}%`}></div>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>
</section>

<style>
  .first-run-panel-shell {
    position: relative;
    overflow: hidden;
    padding: 0;
    gap: 0;
    isolation: isolate;
    transform: translate3d(0, 0, 0);
    opacity: 1;
    transition:
      transform 760ms cubic-bezier(0.22, 1, 0.36, 1),
      opacity 560ms ease;
    will-change: transform, opacity;
  }

  .first-run-viewport {
    position: relative;
    flex: 1;
    min-height: 0;
    overflow: hidden;
    border-radius: inherit;
  }

  .first-run-scene-stack {
    display: grid;
    grid-template-rows: minmax(0, 1fr) minmax(0, 1fr);
    height: 200%;
    transform: translate3d(0, 0, 0);
    transition: transform 860ms cubic-bezier(0.22, 1, 0.36, 1);
    will-change: transform;
  }

  .first-run-panel-shell.is-download-stage .first-run-scene-stack {
    transform: translate3d(0, -50%, 0);
  }

  .first-run-mask.is-exiting-home {
    pointer-events: none;
  }

  .first-run-mask.is-exiting-home .first-run-stage {
    background: color-mix(in srgb, var(--bg-base, #0c0e14) 22%, transparent);
    backdrop-filter: blur(0);
    -webkit-backdrop-filter: blur(0);
    transition:
      background 760ms ease,
      backdrop-filter 760ms ease,
      -webkit-backdrop-filter 760ms ease;
  }

  .first-run-panel-shell.is-exiting-home {
    transform: translate3d(0, -11%, 0);
    opacity: 0;
  }

  .first-run-panel-shell.is-exiting-home .first-run-scene-stack {
    transform: translate3d(0, -66%, 0);
  }

  .first-run-scene {
    position: relative;
    min-height: 0;
    overflow: hidden;
  }

  .first-run-mask {
    position: fixed;
    inset: 0;
    z-index: var(--z-onboarding);
    pointer-events: none;
  }

  .first-run-drag-region {
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    height: 48px;
    z-index: 1;
    pointer-events: auto;
    -webkit-app-region: drag;
  }

  .first-run-stage {
    position: absolute;
    top: var(--top-inset, 0px);
    right: 0;
    bottom: 0;
    left: 0;
    display: flex;
    justify-content: center;
    padding: 24px;
    background: var(--bg-base, #0c0e14);
    backdrop-filter: blur(22px);
    -webkit-backdrop-filter: blur(22px);
    overflow: hidden;
    pointer-events: auto;
  }

  .first-run-panel {
    width: min(1440px, 100%);
    display: flex;
    flex-direction: column;
    gap: 24px;
    padding: 32px 34px;
    border-radius: var(--radius-xl, 20px);
  }

  .first-run-panel-select {
    gap: 22px;
  }

  .first-run-select-layer {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: 22px;
    height: 100%;
    min-height: 0;
    overflow: auto;
    padding: 32px 34px;
    transition:
      opacity 420ms ease,
      filter 520ms cubic-bezier(0.22, 1, 0.36, 1);
    opacity: 1;
    filter: blur(0);
    will-change: opacity, filter;
  }

  .first-run-panel-shell.is-download-stage .first-run-select-layer {
    opacity: 0.12;
    filter: blur(10px);
    pointer-events: none;
  }

  .first-run-download-layer {
    position: relative;
    height: 100%;
    overflow: hidden;
  }

  .download-background {
    position: absolute;
    inset: 0;
    opacity: 0;
    transition: opacity 540ms ease;
  }

  .first-run-panel-shell.is-download-stage .download-background {
    opacity: 1;
  }

  .hero-copy {
    display: flex;
    flex-direction: column;
    gap: 10px;
    max-width: 760px;
  }

  .first-run-head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 16px;
  }

  .skip-button {
    flex-shrink: 0;
    border: 1px solid var(--border);
    background: transparent;
    color: var(--text-secondary);
    font: inherit;
    font-size: var(--font-sm);
    padding: 8px 14px;
    border-radius: var(--radius-pill);
    cursor: pointer;
    transition:
      background var(--transition-fast),
      border-color var(--transition-fast),
      color var(--transition-fast);
  }

  .skip-button:hover {
    background: var(--bg-surface);
    border-color: var(--border-focus);
    color: var(--text-primary);
  }

  .hero-copy h1 {
    font-size: clamp(1.7rem, 3.6vw, 2.7rem);
    line-height: 1.08;
    letter-spacing: -0.03em;
    color: var(--text-primary);
  }

  .hero-copy p {
    max-width: 720px;
    color: var(--text-secondary);
    font-size: var(--font-sm);
    line-height: 1.65;
  }

  .model-grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 12px;
    margin-top: 20px;
  }

  .model-card {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 16px 14px;
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    background: var(--bg-surface);
    color: inherit;
    text-align: left;
    cursor: pointer;
    transition:
      transform var(--transition-normal, 180ms ease),
      border-color var(--transition-normal, 180ms ease),
      background var(--transition-normal, 180ms ease),
      box-shadow var(--transition-normal, 180ms ease);
  }

  .model-card:hover {
    transform: translateY(-2px);
    border-color: var(--accent-border);
    background: var(--bg-surface-hover);
    box-shadow: var(--shadow-md);
  }

  .model-card-top {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 8px;
  }

  .model-card-title-row {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    align-items: center;
    min-width: 0;
  }

  .model-card-title {
    font-size: var(--font-lg);
    font-weight: 700;
    color: var(--text-primary);
    letter-spacing: -0.01em;
  }

  .model-card-hint {
    color: var(--accent);
    font-size: var(--font-xs);
    font-weight: 600;
    line-height: 1.45;
  }

  .model-card-desc {
    color: var(--text-secondary);
    font-size: var(--font-2xs);
    line-height: 1.55;
  }

  .pill {
    display: inline-flex;
    align-items: center;
    padding: 3px 8px;
    border-radius: var(--radius-pill);
    font-size: var(--font-2xs);
    font-weight: 600;
  }

  .pill-success {
    background: var(--success-soft);
    color: var(--success);
  }

  .download-globe-shell {
    position: absolute;
    inset: 0;
    z-index: 1;
    transform: translate3d(0, 18%, 0) scale(0.985);
    opacity: 0;
    transition:
      transform 880ms cubic-bezier(0.16, 1, 0.3, 1),
      opacity 520ms ease;
    will-change: transform, opacity;
  }

  .first-run-panel-shell.is-download-stage .download-globe-shell {
    transform: translate3d(0, 0, 0) scale(1);
    opacity: 1;
    transition-delay: 70ms;
  }

  .download-canvas-layer {
    position: absolute;
    inset: 0;
  }

  .download-overlay {
    position: relative;
    z-index: 2;
    display: flex;
    flex-direction: column;
    height: 100%;
    pointer-events: none;
  }

  .download-overlay > * {
    pointer-events: auto;
  }

  .download-content {
    position: relative;
    flex: 1;
    display: flex;
    flex-direction: column;
    justify-content: flex-end;
    padding: 32px 34px 0;
  }

  .download-info {
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
    gap: 8px;
    padding: 0 16px;
    opacity: 0;
    transform: translate3d(0, 42px, 0);
    transition:
      transform 620ms cubic-bezier(0.22, 1, 0.36, 1),
      opacity 420ms ease;
    will-change: transform, opacity;
  }

  .first-run-panel-shell.is-download-stage .download-info {
    opacity: 1;
    transform: translate3d(0, 0, 0);
    transition-delay: 220ms;
  }

  .first-run-panel-shell.is-ready-stage .download-info {
    opacity: 0;
    transform: translate3d(0, 18px, 0);
    transition-delay: 0ms;
  }

  .download-title {
    font-size: var(--font-lg);
    font-weight: 700;
    color: var(--text-primary);
    letter-spacing: -0.01em;
  }

  .download-msg {
    color: var(--text-secondary);
    font-size: var(--font-sm);
    line-height: 1.6;
  }

  .download-cta {
    position: absolute;
    right: 10%;
    top: 50%;
    display: flex;
    flex-direction: column;
    gap: 20px;
    opacity: 0;
    pointer-events: none;
    transform: translate3d(24px, -50%, 0);
    transition:
      transform 620ms cubic-bezier(0.22, 1, 0.36, 1),
      opacity 420ms ease;
  }

  .first-run-panel-shell.is-ready-stage .download-cta {
    opacity: 1;
    pointer-events: auto;
    transform: translate3d(0, -50%, 0);
    transition-delay: 220ms;
  }

  .cta-title {
    font-size: clamp(1.2rem, 2.4vw, 1.6rem);
    font-weight: 700;
    color: var(--text-primary);
    letter-spacing: -0.02em;
    line-height: 1.2;
  }

  .cta-desc {
    color: var(--text-secondary);
    font-size: var(--font-sm);
    line-height: 1.6;
    max-width: 240px;
  }

  .cta-btn {
    padding: 12px 28px;
    border-radius: var(--radius-md);
    font-size: var(--font-md);
    align-self: flex-start;
  }

  .download-error {
    display: flex;
    flex-direction: column;
    gap: 14px;
    margin-top: 18px;
    padding: 18px;
    border-radius: var(--radius-lg);
    border: 1px solid var(--danger-border);
    background: var(--danger-soft);
  }

  .download-actions {
    display: flex;
    gap: 10px;
    flex-wrap: wrap;
  }

  .progress-footer {
    position: relative;
    z-index: 2;
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 0 34px 32px;
    margin-top: auto;
    opacity: 0;
    pointer-events: none;
    transform: translate3d(0, 52px, 0);
    transition:
      transform 620ms cubic-bezier(0.22, 1, 0.36, 1),
      opacity 420ms ease;
    will-change: transform, opacity;
  }

  .first-run-panel-shell.is-download-stage:not(.is-ready-stage) .progress-footer {
    opacity: 1;
    pointer-events: auto;
    transform: translate3d(0, 0, 0);
    transition-delay: 260ms;
  }

  .progress-meta {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    color: var(--text-secondary);
  }

  .progress-meta strong {
    color: var(--text-primary);
    font-size: var(--font-lg);
  }

  .progress-track {
    position: relative;
    height: 8px;
    border-radius: var(--radius-pill);
    overflow: hidden;
    background: var(--bg-inset);
  }

  .progress-fill {
    position: absolute;
    inset: 0 auto 0 0;
    border-radius: inherit;
    background: var(--accent);
    transition: width 0.4s ease;
  }


  @media (max-width: 820px) {
    .first-run-stage {
      align-items: stretch;
      padding: 18px;
    }

    .first-run-select-layer,
    .download-content {
      padding: 24px 22px 0;
    }

    .progress-footer {
      padding: 0 22px 24px;
    }

    .model-grid {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }

    .first-run-head {
      flex-direction: column;
      align-items: stretch;
    }

    .skip-button {
      align-self: flex-end;
    }
  }

  @media (max-width: 640px) {
    .first-run-stage {
      padding: 14px;
    }

    .first-run-select-layer,
    .download-content {
      padding: 20px 18px 0;
    }

    .progress-footer {
      padding: 0 18px 20px;
    }

    .model-grid {
      grid-template-columns: 1fr;
    }

    .download-cta {
      right: 18px;
      left: 18px;
    }
  }
</style>
