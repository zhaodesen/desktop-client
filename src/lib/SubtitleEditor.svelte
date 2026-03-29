<script lang="ts">
  import type { SubtitleDocument, SubtitleCue } from "../shared/types";
  import { formatDuration } from "../shared/utils";

  interface Props {
    document: SubtitleDocument | undefined;
    lastMainPage: string;
    saveNotice?: string;
    isSaving?: boolean;
    onBack: () => void;
    onSave: () => void;
    onCueChange: (index: number, field: "text" | "secondaryText", value: string) => void;
  }

  const { document, lastMainPage, saveNotice, isSaving = false, onBack, onSave, onCueChange }: Props = $props();

  function formatCueTime(cue: SubtitleCue): string {
    return `${formatDuration(cue.startMs)} - ${formatDuration(cue.endMs)}`;
  }
</script>

<section class="page" data-active="true">
  <header class="page-header page-header-editor">
    <div>
      <h2>字幕详情</h2>
      <p class="text-dim text-xs">
        {document ? `${document.title} · ${document.cues.length} 条字幕` : "选择素材后可在这里校对原文和中文字幕"}
      </p>
    </div>
    <div class="header-actions">
      <button class="btn btn-outline" type="button" onclick={onBack}>返回</button>
      <button class="btn btn-primary" type="button" disabled={isSaving} onclick={onSave}>
        {isSaving ? "保存中…" : "保存"}
      </button>
    </div>
  </header>

  {#if saveNotice}
    <div class="subtitle-save-notice" role="status">
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <path d="M20 6 9 17l-5-5"/>
      </svg>
      <span>{saveNotice}</span>
    </div>
  {/if}

  <div class="card subtitle-editor-card">
    <div class="subtitle-editor-head">
      <span class="subtitle-editor-col">原文</span>
      <span class="subtitle-editor-col">中文字幕</span>
    </div>

    {#if !document}
      <div class="subtitle-editor-list empty-state">
        <div class="empty-content">
          <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" opacity="0.4"><path d="M4 19.5A2.5 2.5 0 0 1 6.5 17H20"/><path d="M6.5 2H20v20H6.5A2.5 2.5 0 0 1 4 19.5v-15A2.5 2.5 0 0 1 6.5 2z"/></svg>
          <span>字幕详情会显示在这里</span>
        </div>
      </div>
    {:else}
      <div class="subtitle-editor-list">
        {#each document.cues as cue, index (index)}
          <div class="subtitle-editor-row">
            <div class="subtitle-editor-time">{formatCueTime(cue)}</div>
            <div class="subtitle-editor-field">
              <label for="cue-text-{index}">原文</label>
              <textarea
                id="cue-text-{index}"
                value={cue.text}
                oninput={(e) => onCueChange(index, "text", (e.target as HTMLTextAreaElement).value)}
              ></textarea>
            </div>
            <div class="subtitle-editor-field">
              <label for="cue-secondary-{index}">中文字幕</label>
              <textarea
                id="cue-secondary-{index}"
                value={cue.secondaryText ?? ""}
                oninput={(e) => onCueChange(index, "secondaryText", (e.target as HTMLTextAreaElement).value)}
              ></textarea>
            </div>
          </div>
        {/each}
      </div>
    {/if}
  </div>
</section>

<style>
  .subtitle-save-notice {
    position: fixed;
    top: 64px;
    left: calc(var(--sidebar-w) + ((100vw - var(--sidebar-w)) / 2));
    transform: translateX(-50%);
    z-index: var(--z-toast);
    display: inline-flex;
    align-items: center;
    gap: 8px;
    padding: 8px 12px;
    max-width: min(520px, calc(100vw - var(--sidebar-w) - 48px));
    border-radius: var(--radius-pill);
    background: var(--success-soft);
    color: var(--success);
    border: 1px solid rgba(52, 211, 153, 0.18);
    font-size: var(--font-xs);
    font-weight: 600;
    box-shadow: var(--shadow-md);
    backdrop-filter: blur(8px);
    -webkit-backdrop-filter: blur(8px);
    animation: subtitle-save-notice-in 160ms ease;
  }

  @keyframes subtitle-save-notice-in {
    from { opacity: 0; transform: translateX(-50%) translateY(-4px); }
    to { opacity: 1; transform: translateX(-50%) translateY(0); }
  }
</style>
