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
  <dialog open class="confirm-dialog">
    <div class="dialog-body">
      <h3>{title}</h3>
      <p>{message}</p>
      <div class="dialog-actions">
        <button class="btn btn-outline" type="button" onclick={onCancel}>取消</button>
        <button class="btn btn-danger-solid" type="button" onclick={onConfirm}>确认</button>
      </div>
    </div>
  </dialog>
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div class="dialog-backdrop" role="presentation" onclick={onCancel}></div>
{/if}

<style>
  .dialog-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.6);
    z-index: 99;
  }
  .confirm-dialog {
    position: fixed;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    z-index: 100;
    border: none;
    border-radius: var(--radius-lg);
    background: var(--bg-raised);
    color: var(--text-primary);
    padding: 0;
    max-width: 380px;
    width: 90vw;
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.5);
  }
</style>
