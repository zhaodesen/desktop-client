<script lang="ts">
  import type { ModelInfo, ModelStatus } from "../shared/types";

  type OnboardingStep = "select-model" | "downloading" | "ready";

  type ModelGuide = {
    pros: string[];
    cons: string[];
    recommended?: boolean;
  };

  interface Props {
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
    onStart: () => void;
  }

  const {
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
    onStart,
  }: Props = $props();

  const selectedModel = $derived(models.find((model) => model.id === selectedModelId));
</script>

<section class="first-run-mask" aria-label="首次启动引导">
  {#if step === "select-model"}
    <div class="first-run-panel">
      <div class="hero-copy">
        <span class="eyebrow">已准备就绪</span>
        <h1>在开始之前，请先选择一个识别模型</h1>
        <p>
          这里选择的就是设置里的离线模型，后续会用它对音视频进行解析。首次选择后会自动下载到本机，
          稍后也可以在设置里重新切换。
        </p>
      </div>

      <div class="model-grid">
        {#each models as model (model.id)}
          {@const guide = modelGuides[model.id]}
          {@const status = modelsStatusMap.get(model.id)}
          <button
            class="model-card"
            class:model-card-recommended={guide?.recommended}
            type="button"
            onclick={() => onSelectModel(model.id)}
          >
            <div class="model-card-head">
              <div>
                <div class="model-card-title">
                  {model.label}
                  {#if guide?.recommended}
                    <span class="pill pill-accent">推荐</span>
                  {/if}
                  {#if status?.installed}
                    <span class="pill pill-success">已下载</span>
                  {/if}
                </div>
                <p class="model-card-desc">{model.description}</p>
              </div>
              <span class="model-card-size">{model.sizeMb} MB</span>
            </div>

            <div class="guide-block">
              <h3>优点</h3>
              <p>{guide?.pros?.join(" · ") ?? "速度和效果均衡"}</p>
            </div>

            <div class="guide-block">
              <h3>缺点</h3>
              <p>{guide?.cons?.join(" · ") ?? "模型越大，占用空间越高"}</p>
            </div>

            <div class="model-card-action">
              {#if status?.installed}
                直接使用这个模型
              {:else}
                选择并开始下载
              {/if}
            </div>
          </button>
        {/each}
      </div>
    </div>
  {:else if step === "downloading"}
    <div class="first-run-panel first-run-panel-narrow">
      <div class="download-stage">
        <span class="eyebrow">正在下载</span>
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
      <span class="eyebrow">已准备就绪</span>
      <h1>{selectedModel?.label ?? "识别模型"} 已下载完成</h1>
      <p>
        现在可以开始导入音频或视频生成字幕了。后续若要更换模型，随时可以到“设置 / 离线模型”里调整。
      </p>
      <button class="btn btn-primary onboarding-start-btn" type="button" onclick={onStart}>开始使用</button>

      <div class="progress-footer">
        <div class="progress-meta">
          <span>下载进度</span>
          <strong>100%</strong>
        </div>
        <div class="progress-track">
          <div class="progress-fill" style="width: 100%"></div>
        </div>
      </div>
    </div>
  {/if}
</section>

<style>
  .first-run-mask {
    position: fixed;
    inset: 0;
    z-index: var(--z-onboarding);
    display: flex;
    align-items: stretch;
    justify-content: center;
    padding: 28px;
    background: var(--bg-base, #0c0e14);
    backdrop-filter: blur(22px);
    -webkit-backdrop-filter: blur(22px);
  }

  .first-run-panel {
    width: min(1180px, 100%);
    min-height: 100%;
    display: flex;
    flex-direction: column;
    gap: 32px;
    padding: 48px;
    border-radius: var(--radius-xl, 20px);
    border: 1px solid var(--border);
    background: var(--bg-raised);
    box-shadow: var(--shadow-lg);
  }

  .first-run-panel-narrow {
    max-width: 780px;
    justify-content: space-between;
  }

  .first-run-panel-ready {
    align-items: center;
    justify-content: center;
    text-align: center;
  }

  .hero-copy {
    display: flex;
    flex-direction: column;
    gap: 14px;
    max-width: 760px;
  }

  .hero-copy h1,
  .download-stage h1 {
    font-size: clamp(1.8rem, 4vw, 2.8rem);
    line-height: 1.1;
    letter-spacing: -0.02em;
    color: var(--text-primary);
  }

  .hero-copy p,
  .download-stage p,
  .first-run-panel-ready p {
    max-width: 720px;
    color: var(--text-secondary);
    font-size: 0.95rem;
    line-height: 1.75;
  }

  .eyebrow {
    display: inline-flex;
    align-items: center;
    width: fit-content;
    padding: 5px 12px;
    border-radius: var(--radius-pill);
    background: var(--accent-soft);
    color: var(--accent);
    font-size: 0.8rem;
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }

  .model-grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 12px;
  }

  .model-card {
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding: 20px;
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    background: var(--bg-surface);
    color: inherit;
    text-align: left;
    cursor: pointer;
    transition: transform var(--transition-normal, 180ms ease), border-color var(--transition-normal, 180ms ease), background var(--transition-normal, 180ms ease), box-shadow var(--transition-normal, 180ms ease);
  }

  .model-card:hover {
    transform: translateY(-3px);
    border-color: var(--accent-border);
    background: var(--bg-surface-hover);
    box-shadow: var(--shadow-md);
  }

  .model-card-recommended {
    border-color: var(--accent-border);
  }

  .model-card-head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 16px;
  }

  .model-card-title {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    align-items: center;
    font-size: 1rem;
    font-weight: 700;
    color: var(--text-primary);
  }

  .model-card-desc {
    margin-top: 6px;
    color: var(--text-secondary);
    font-size: 0.86rem;
    line-height: 1.65;
  }

  .model-card-size {
    color: var(--text-dim);
    font-size: 0.78rem;
    white-space: nowrap;
  }

  .guide-block {
    display: flex;
    flex-direction: column;
    gap: 5px;
    padding: 10px 12px;
    border-radius: var(--radius-md);
    background: var(--bg-inset);
  }

  .guide-block h3 {
    font-size: 0.74rem;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--text-dim);
  }

  .guide-block p {
    color: var(--text-secondary);
    font-size: 0.86rem;
    line-height: 1.65;
  }

  .model-card-action {
    margin-top: auto;
    font-size: 0.84rem;
    font-weight: 600;
    color: var(--accent);
  }

  .pill {
    display: inline-flex;
    align-items: center;
    padding: 3px 8px;
    border-radius: var(--radius-pill);
    font-size: 0.72rem;
    font-weight: 600;
  }

  .pill-accent {
    background: var(--accent-soft);
    color: var(--accent);
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
    font-size: 1.05rem;
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
    font-size: 0.92rem;
    margin-top: 10px;
  }

  @media (max-width: 920px) {
    .first-run-mask { padding: 16px; }
    .first-run-panel { padding: 28px 22px; }
    .model-grid { grid-template-columns: 1fr; }
  }
</style>
