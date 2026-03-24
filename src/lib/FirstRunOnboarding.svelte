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
    z-index: 90;
    display: flex;
    align-items: stretch;
    justify-content: center;
    padding: 28px;
    background:
      radial-gradient(circle at top left, rgba(232, 148, 78, 0.18), transparent 28%),
      radial-gradient(circle at bottom right, rgba(78, 114, 232, 0.14), transparent 30%),
      rgba(7, 10, 14, 0.98);
    backdrop-filter: blur(22px);
  }

  .first-run-panel {
    width: min(1180px, 100%);
    min-height: 100%;
    display: flex;
    flex-direction: column;
    gap: 28px;
    padding: 52px;
    border-radius: 28px;
    border: 1px solid rgba(255, 255, 255, 0.08);
    background:
      linear-gradient(160deg, rgba(255, 255, 255, 0.05), rgba(255, 255, 255, 0.02)),
      rgba(11, 14, 20, 0.94);
    box-shadow: 0 28px 80px rgba(0, 0, 0, 0.45);
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
    font-size: clamp(2rem, 4vw, 3.2rem);
    line-height: 1.08;
    letter-spacing: -0.03em;
  }

  .hero-copy p,
  .download-stage p,
  .first-run-panel-ready p {
    max-width: 720px;
    color: rgba(232, 228, 220, 0.74);
    font-size: 1rem;
    line-height: 1.75;
  }

  .eyebrow {
    display: inline-flex;
    align-items: center;
    width: fit-content;
    padding: 6px 12px;
    border-radius: 999px;
    background: rgba(232, 148, 78, 0.12);
    color: #f3ba85;
    font-size: 0.82rem;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }

  .model-grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 16px;
  }

  .model-card {
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding: 20px;
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 20px;
    background: rgba(255, 255, 255, 0.03);
    color: inherit;
    text-align: left;
    cursor: pointer;
    transition: transform 160ms ease, border-color 160ms ease, background 160ms ease;
  }

  .model-card:hover {
    transform: translateY(-3px);
    border-color: rgba(232, 148, 78, 0.42);
    background: rgba(255, 255, 255, 0.05);
  }

  .model-card-recommended {
    border-color: rgba(232, 148, 78, 0.3);
    box-shadow: inset 0 0 0 1px rgba(232, 148, 78, 0.1);
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
    font-size: 1.05rem;
    font-weight: 700;
  }

  .model-card-desc {
    margin-top: 8px;
    color: rgba(232, 228, 220, 0.64);
    font-size: 0.88rem;
    line-height: 1.65;
  }

  .model-card-size {
    color: rgba(232, 228, 220, 0.54);
    font-size: 0.78rem;
    white-space: nowrap;
  }

  .guide-block {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 12px 14px;
    border-radius: 14px;
    background: rgba(255, 255, 255, 0.03);
  }

  .guide-block h3 {
    font-size: 0.78rem;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: rgba(232, 228, 220, 0.5);
  }

  .guide-block p {
    color: rgba(232, 228, 220, 0.84);
    font-size: 0.88rem;
    line-height: 1.7;
  }

  .model-card-action {
    margin-top: auto;
    font-size: 0.86rem;
    font-weight: 600;
    color: #f3ba85;
  }

  .pill {
    display: inline-flex;
    align-items: center;
    padding: 3px 8px;
    border-radius: 999px;
    font-size: 0.72rem;
    font-weight: 600;
  }

  .pill-accent {
    background: rgba(232, 148, 78, 0.16);
    color: #f3ba85;
  }

  .pill-success {
    background: rgba(74, 222, 128, 0.12);
    color: #86efac;
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
    border-radius: 18px;
    border: 1px solid rgba(248, 113, 113, 0.24);
    background: rgba(248, 113, 113, 0.08);
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
    color: rgba(232, 228, 220, 0.68);
  }

  .progress-meta strong {
    color: rgba(255, 255, 255, 0.94);
    font-size: 1.1rem;
  }

  .progress-track {
    position: relative;
    height: 10px;
    border-radius: 999px;
    overflow: hidden;
    background: rgba(255, 255, 255, 0.08);
  }

  .progress-fill {
    position: absolute;
    inset: 0 auto 0 0;
    border-radius: inherit;
    background: linear-gradient(90deg, #e8944e, #f3ba85);
    box-shadow: 0 0 24px rgba(232, 148, 78, 0.4);
  }

  .ready-icon {
    display: grid;
    place-items: center;
    width: 84px;
    height: 84px;
    border-radius: 50%;
    margin-bottom: 8px;
    background: rgba(74, 222, 128, 0.12);
    color: #86efac;
  }

  .onboarding-start-btn {
    padding: 12px 28px;
    border-radius: 14px;
    font-size: 0.95rem;
    margin-top: 10px;
  }

  @media (max-width: 920px) {
    .first-run-mask {
      padding: 16px;
    }

    .first-run-panel {
      padding: 28px 22px;
      border-radius: 22px;
    }

    .model-grid {
      grid-template-columns: 1fr;
    }
  }
</style>
