<script lang="ts">
  let visible = $state(false);
  let title = $state("确认操作");
  let message = $state("确定要执行此操作吗？");
  let resolve: ((v: boolean) => void) | null = null;

  export function show(t: string, m: string): Promise<boolean> {
    title = t;
    message = m;
    visible = true;
    return new Promise((r) => {
      resolve = r;
    });
  }

  function onConfirm() {
    visible = false;
    resolve?.(true);
    resolve = null;
  }

  function onCancel() {
    visible = false;
    resolve?.(false);
    resolve = null;
  }
</script>

{#if visible}
  <div class="confirm-dialog-shell" role="presentation">
    <div
      class="confirm-dialog"
      role="alertdialog"
      aria-modal="true"
      aria-labelledby="confirm-dialog-title"
      aria-describedby="confirm-dialog-message"
    >
      <div class="dialog-body">
        <h3 id="confirm-dialog-title">{title}</h3>
        <p id="confirm-dialog-message">{message}</p>
        <div class="dialog-actions">
          <button class="btn btn-outline" type="button" onclick={onCancel}>取消</button>
          <button class="btn btn-danger-solid" type="button" onclick={onConfirm}>确认</button>
        </div>
      </div>
    </div>
  </div>
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div class="dialog-backdrop" role="presentation" onclick={onCancel}></div>
{/if}

<style>
  .dialog-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.45);
    backdrop-filter: blur(6px);
    -webkit-backdrop-filter: blur(6px);
    z-index: var(--z-confirm);
    animation: backdrop-in 0.15s ease;
  }

  .confirm-dialog-shell {
    position: fixed;
    inset: 0;
    display: grid;
    place-items: center;
    padding: 24px;
    z-index: calc(var(--z-confirm) + 1);
    pointer-events: none;
  }

  @keyframes backdrop-in {
    from { opacity: 0; }
    to { opacity: 1; }
  }

  .confirm-dialog {
    position: relative;
    pointer-events: auto;
    border: none;
    border-radius: var(--radius-xl, 20px);
    background: var(--bg-raised);
    color: var(--text-primary);
    padding: 0;
    max-width: 380px;
    width: 90vw;
    box-shadow: var(--shadow-lg);
    animation: dialog-pop 0.2s ease;
  }

  @keyframes dialog-pop {
    from { opacity: 0; transform: translateY(8px) scale(0.97); }
    to { opacity: 1; transform: translateY(0) scale(1); }
  }
</style>
