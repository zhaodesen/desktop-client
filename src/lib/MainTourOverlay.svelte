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
    /* Allow clicks on the card but block outside */
    pointer-events: none;
  }

  .tour-backdrop {
    position: absolute;
    inset: 0;
    background: rgba(0, 0, 0, 0.55);
    pointer-events: auto;
  }

  .tour-highlight {
    position: fixed;
    border-radius: var(--radius-lg, 16px);
    border: 2px solid var(--accent, #d48a42);
    box-shadow:
      0 0 0 9999px rgba(0, 0, 0, 0.55),
      0 0 0 6px rgba(212, 138, 66, 0.1);
    pointer-events: none;
    z-index: 81;
  }

  .tour-card {
    position: fixed;
    width: min(360px, calc(100vw - 40px));
    padding: 22px;
    border-radius: var(--radius-lg, 16px);
    border: 1px solid var(--border, rgba(255, 255, 255, 0.08));
    background: var(--bg-raised, #161921);
    color: var(--text-primary, #e4e2db);
    box-shadow: var(--shadow-lg, 0 12px 40px rgba(0, 0, 0, 0.45));
    pointer-events: auto;
    z-index: 82;
  }

  .tour-card-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    margin-bottom: 10px;
  }

  .tour-step-index {
    color: var(--text-dim, rgba(228, 226, 219, 0.36));
    font-size: 0.78rem;
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }

  .tour-skip {
    border: none;
    background: transparent;
    color: var(--text-secondary, rgba(228, 226, 219, 0.62));
    font: inherit;
    font-size: 0.84rem;
    cursor: pointer;
    padding: 4px 8px;
    border-radius: var(--radius-xs, 6px);
    transition: color 150ms, background 150ms;
  }

  .tour-skip:hover {
    color: var(--text-primary);
    background: var(--bg-surface);
  }

  .tour-card h2 {
    font-size: 1.12rem;
    font-weight: 700;
    margin-bottom: 8px;
    color: var(--text-primary);
  }

  .tour-card p {
    color: var(--text-secondary);
    line-height: 1.7;
    font-size: 0.88rem;
  }

  .tour-hint {
    margin-top: 12px;
    padding: 10px 14px;
    border-radius: var(--radius-md, 12px);
    background: var(--accent-soft, rgba(212, 138, 66, 0.1));
    color: var(--accent, #d48a42);
    font-size: 0.84rem;
    line-height: 1.65;
  }

  .tour-actions {
    display: flex;
    justify-content: flex-end;
    margin-top: 16px;
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
