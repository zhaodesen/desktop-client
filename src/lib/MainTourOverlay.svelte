<script lang="ts">
  type TourTargetRect = {
    top: number;
    left: number;
    right: number;
    bottom: number;
    width: number;
    height: number;
  };

  type TourStep = {
    id: string;
    title: string;
    description: string;
    hint: string;
  };

  interface Props {
    step: TourStep;
    index: number;
    total: number;
    targetRect: TourTargetRect | undefined;
    onSkip: () => void;
    onNext: () => void;
    onFinish: () => void;
  }

  const { step, index, total, targetRect, onSkip, onNext, onFinish }: Props = $props();

  const tooltipStyle = $derived.by(() => {
    if (!targetRect) {
      return "top: 96px; left: 260px;";
    }
    const top = Math.max(24, targetRect.top - 12);
    const left = targetRect.right + 24;
    return `top: ${top}px; left: ${left}px;`;
  });

  const highlightStyle = $derived.by(() => {
    if (!targetRect) {
      return "display: none;";
    }
    return [
      `top: ${targetRect.top - 6}px`,
      `left: ${targetRect.left - 6}px`,
      `width: ${targetRect.width + 12}px`,
      `height: ${targetRect.height + 12}px`,
    ].join("; ");
  });
</script>

<section class="tour-mask" aria-label="新手引导">
  <div class="tour-backdrop"></div>
  <div class="tour-highlight" style={highlightStyle}></div>

  <div class="tour-card" style={tooltipStyle} role="dialog" aria-modal="true">
    <div class="tour-card-head">
      <span class="tour-step-index">步骤 {index + 1} / {total}</span>
      <button class="tour-skip" type="button" onclick={onSkip}>跳过</button>
    </div>

    <h2>{step.title}</h2>
    <p>{step.description}</p>
    <div class="tour-hint">{step.hint}</div>

    <div class="tour-actions">
      {#if index + 1 < total}
        <button class="btn btn-primary" type="button" onclick={onNext}>下一步</button>
      {:else}
        <button class="btn btn-primary" type="button" onclick={onFinish}>开始使用</button>
      {/if}
    </div>
  </div>
</section>

<style>
  .tour-mask {
    position: fixed;
    inset: 0;
    z-index: 80;
    pointer-events: none;
  }

  .tour-backdrop {
    position: absolute;
    inset: 0;
    background: rgba(5, 8, 12, 0.72);
    backdrop-filter: blur(3px);
  }

  .tour-highlight {
    position: fixed;
    border-radius: 16px;
    border: 1px solid rgba(232, 148, 78, 0.82);
    box-shadow:
      0 0 0 9999px rgba(5, 8, 12, 0.72),
      0 0 0 8px rgba(232, 148, 78, 0.12),
      0 18px 46px rgba(0, 0, 0, 0.28);
    pointer-events: none;
  }

  .tour-card {
    position: fixed;
    width: min(360px, calc(100vw - 40px));
    padding: 22px;
    border-radius: 20px;
    border: 1px solid rgba(255, 255, 255, 0.08);
    background: rgba(15, 19, 26, 0.96);
    box-shadow: 0 24px 64px rgba(0, 0, 0, 0.42);
    pointer-events: auto;
  }

  .tour-card-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    margin-bottom: 10px;
  }

  .tour-step-index {
    color: rgba(232, 228, 220, 0.58);
    font-size: 0.8rem;
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }

  .tour-skip {
    border: none;
    background: transparent;
    color: rgba(232, 228, 220, 0.72);
    font: inherit;
    cursor: pointer;
  }

  .tour-card h2 {
    font-size: 1.18rem;
    margin-bottom: 10px;
  }

  .tour-card p {
    color: rgba(232, 228, 220, 0.74);
    line-height: 1.7;
  }

  .tour-hint {
    margin-top: 14px;
    padding: 12px 14px;
    border-radius: 14px;
    background: rgba(232, 148, 78, 0.09);
    color: #f3ba85;
    font-size: 0.86rem;
    line-height: 1.65;
  }

  .tour-actions {
    display: flex;
    justify-content: flex-end;
    margin-top: 18px;
  }

  @media (max-width: 900px) {
    .tour-card {
      left: 20px !important;
      right: 20px;
      top: auto !important;
      bottom: 20px;
      width: auto;
    }
  }
</style>
