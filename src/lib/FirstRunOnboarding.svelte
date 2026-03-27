<script lang="ts">
  import type { ModelInfo, ModelStatus } from "../shared/types";

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

  const modelHints: Record<string, string> = {
    tiny: "最快最轻，低配优先",
    base: "日常首选，均衡稳定",
    small: "更高准确率",
    medium: "长音频更稳",
    "large-v3-turbo": "最高质量",
  };
</script>

<section class="first-run-mask" style={`--top-inset: ${topInset}px;`} aria-label="首次启动引导">
  <div class="first-run-stage">
    {#if step === "select-model"}
      <div class="first-run-panel first-run-panel-select">
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
    {:else if step === "downloading"}
      <div class="first-run-panel first-run-panel-narrow">
        <div class="download-stage">
          <h1>{selectedModel?.label ?? "识别模型"}</h1>
          <p>{downloadMessage}</p>

          {#if error}
            <div class="download-error">
              <p>{error}</p>
              <div class="download-actions">
                <button class="btn btn-ghost" type="button" onclick={onBack}>返回重选</button>
                <button class="btn btn-primary" type="button" onclick={onRetry}>重新下载</button>
              </div>
            </div>
          {/if}
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
    {:else}
      <div class="first-run-panel first-run-panel-narrow first-run-panel-ready">
        <div class="ready-icon" aria-hidden="true">
          <svg width="38" height="38" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round">
            <polyline points="20 6 9 17 4 12" />
          </svg>
        </div>
        <h1>{selectedModel?.label ?? "识别模型"} 已下载完成</h1>
        <p>现在可以开始导入音频或视频生成字幕了，后续仍可在设置中切换模型。</p>
        <button class="btn btn-primary onboarding-start-btn" type="button" onclick={onStart}>开始使用</button>
      </div>
    {/if}
  </div>
</section>

<style>
  .first-run-mask {
    position: fixed;
    inset: 0;
    z-index: var(--z-onboarding);
    pointer-events: none;
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
    overflow: auto;
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

  .first-run-panel-narrow {
    max-width: 780px;
    justify-content: space-between;
    min-height: min(560px, 100%);
  }

  .first-run-panel-ready {
    align-items: center;
    justify-content: center;
    text-align: center;
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

  .hero-copy h1,
  .download-stage h1 {
    font-size: clamp(1.7rem, 3.6vw, 2.7rem);
    line-height: 1.08;
    letter-spacing: -0.03em;
    color: var(--text-primary);
  }

  .hero-copy p,
  .download-stage p,
  .first-run-panel-ready p {
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

  .download-stage {
    display: flex;
    flex-direction: column;
    gap: 14px;
    margin-top: auto;
    margin-bottom: auto;
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
    display: flex;
    flex-direction: column;
    gap: 12px;
    margin-top: auto;
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

  .ready-icon {
    display: grid;
    place-items: center;
    width: 76px;
    height: 76px;
    border-radius: 50%;
    margin-bottom: 8px;
    background: var(--success-soft);
    color: var(--success);
  }

  .onboarding-start-btn {
    padding: 11px 26px;
    border-radius: var(--radius-md);
    font-size: var(--font-md);
    margin-top: 10px;
  }

  @media (max-width: 820px) {
    .first-run-stage {
      align-items: stretch;
      padding: 18px;
    }

    .first-run-panel {
      padding: 24px 22px;
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

    .model-grid {
      grid-template-columns: 1fr;
    }
  }
</style>
