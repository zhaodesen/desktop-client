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
    onTitleChange: (value: string) => void;
    onCueChange: (index: number, field: "text" | "secondaryText", value: string) => void;
  }

  const { document, lastMainPage, saveNotice, isSaving = false, onBack, onSave, onTitleChange, onCueChange }: Props = $props();

  function formatCueTime(cue: SubtitleCue): string {
    return `${formatDuration(cue.startMs)} - ${formatDuration(cue.endMs)}`;
  }

  const cueCount = $derived(document?.cues.length ?? 0);
</script>

<section class="page subtitle-editor-page" data-active="true">
  {#if saveNotice}
    <div class="subtitle-save-notice" role="status">
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <path d="M20 6 9 17l-5-5"/>
      </svg>
      <span>{saveNotice}</span>
    </div>
  {/if}

  {#if document}
    <header class="subtitle-header">
      <div class="subtitle-header-main">
        <div class="subtitle-header-top">
          <span class="subtitle-page-label">字幕详情</span>
          <span class="subtitle-page-meta">{lastMainPage === "resource-list" ? "资源列表" : "播放页"} · {cueCount} 条</span>
        </div>
      </div>

      <div class="subtitle-header-actions">
        <button class="btn btn-outline" type="button" onclick={onBack}>返回</button>
        <button class="btn btn-primary" type="button" disabled={isSaving} onclick={onSave}>
          {isSaving ? "保存中…" : "保存"}
        </button>
      </div>
    </header>

    <section class="subtitle-title-bar">
      <label class="subtitle-title-field">
        <span>标题</span>
        <input
          type="text"
          value={document.title}
          placeholder="请输入字幕标题"
          oninput={(e) => onTitleChange((e.target as HTMLInputElement).value)}
        />
      </label>
    </section>

    <section class="subtitle-workbench">
      <div class="subtitle-grid-head">
        <span>时间</span>
        <span>原文</span>
        <span>中文字幕</span>
      </div>

      <div class="subtitle-editor-list">
        {#each document.cues as cue, index (index)}
          <article class="subtitle-editor-row">
            <div class="subtitle-editor-time">
              <span class="subtitle-editor-index">{String(index + 1).padStart(2, "0")}</span>
              <span class="subtitle-editor-range">{formatCueTime(cue)}</span>
            </div>

            <div class="subtitle-editor-field">
              <label for="cue-text-{index}">原文</label>
              <textarea
                id="cue-text-{index}"
                value={cue.text}
                oninput={(e) => onCueChange(index, "text", (e.target as HTMLTextAreaElement).value)}
              ></textarea>
            </div>

            <div class="subtitle-editor-field">
              <label for="cue-secondary-{index}">
                中文字幕
                {#if !(cue.secondaryText?.trim())}
                  <span class="subtitle-missing-tag">待补全</span>
                {/if}
              </label>
              <textarea
                id="cue-secondary-{index}"
                value={cue.secondaryText ?? ""}
                oninput={(e) => onCueChange(index, "secondaryText", (e.target as HTMLTextAreaElement).value)}
              ></textarea>
            </div>
          </article>
        {/each}
      </div>
    </section>
  {:else}
    <div class="subtitle-empty-state">
      <h2>当前没有可编辑的字幕内容</h2>
      <p>请选择一条已有字幕的素材后再进入此页面。</p>
      <button class="btn btn-primary" type="button" onclick={onBack}>返回上一页</button>
    </div>
  {/if}
</section>

<style>
  .subtitle-editor-page {
    gap: 16px;
  }

  .subtitle-header {
    display: flex;
    justify-content: space-between;
    align-items: flex-end;
    gap: 16px;
    padding-bottom: 14px;
    border-bottom: 1px solid var(--border-subtle);
  }

  .subtitle-header-main {
    display: grid;
    gap: 12px;
    min-width: 0;
    flex: 1;
  }

  .subtitle-header-top {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
    align-items: center;
  }

  .subtitle-page-label {
    font-size: 24px;
    font-weight: 700;
    letter-spacing: -0.03em;
    color: var(--text-primary);
  }

  .subtitle-page-meta {
    font-size: 12px;
    color: var(--text-dim);
  }

  .subtitle-title-bar {
    padding: 14px 16px;
    border-radius: 16px;
    background: var(--bg-surface);
    border: 1px solid var(--border-subtle);
  }

  .subtitle-title-field {
    display: grid;
    gap: 6px;
    max-width: 820px;
  }

  .subtitle-title-field span {
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--text-dim);
  }

  .subtitle-title-field input {
    width: 100%;
    min-height: 44px;
    padding: 0 14px;
    border-radius: 14px;
    border: 1px solid var(--border);
    background: var(--bg-surface);
    color: var(--text-primary);
    font-size: 18px;
    font-weight: 600;
    transition: border-color var(--transition-fast), box-shadow var(--transition-fast);
  }

  .subtitle-title-field input:focus {
    outline: none;
    border-color: var(--accent);
    box-shadow: 0 0 0 3px var(--accent-soft);
  }

  .subtitle-header-actions {
    display: flex;
    gap: 10px;
    align-items: center;
    flex-shrink: 0;
  }

  .subtitle-workbench {
    display: grid;
    gap: 10px;
  }

  .subtitle-grid-head {
    display: grid;
    grid-template-columns: 112px minmax(0, 1fr) minmax(0, 1fr);
    gap: 12px;
    padding: 0 8px;
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--text-dim);
  }

  .subtitle-editor-list {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .subtitle-editor-row {
    display: grid;
    grid-template-columns: 112px minmax(0, 1fr) minmax(0, 1fr);
    gap: 12px;
    padding: 12px;
    border-radius: 16px;
    border: 1px solid var(--border);
    background: var(--bg-surface);
    transition: border-color var(--transition-fast), background var(--transition-fast);
  }

  .subtitle-editor-row:hover {
    border-color: var(--border-focus);
    background: var(--bg-surface-hover);
  }

  .subtitle-editor-time {
    display: grid;
    align-content: start;
    gap: 8px;
    padding-top: 4px;
  }

  .subtitle-editor-index {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: fit-content;
    min-width: 40px;
    padding: 4px 8px;
    border-radius: 999px;
    background: var(--bg-inset);
    color: var(--text-secondary);
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.08em;
  }

  .subtitle-editor-range {
    color: var(--text-dim);
    font-size: 12px;
    line-height: 1.5;
    font-variant-numeric: tabular-nums;
  }

  .subtitle-editor-field {
    display: grid;
    gap: 6px;
  }

  .subtitle-editor-field label {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--text-dim);
  }

  .subtitle-missing-tag {
    display: inline-flex;
    align-items: center;
    padding: 2px 8px;
    border-radius: 999px;
    background: rgba(251, 191, 36, 0.12);
    color: #fbbf24;
    font-size: 10px;
    letter-spacing: 0.06em;
  }

  .subtitle-editor-field textarea {
    min-height: 96px;
    resize: vertical;
    border: 1px solid var(--border);
    border-radius: 12px;
    background: var(--bg-inset);
    color: var(--text-primary);
    font: inherit;
    line-height: 1.6;
    padding: 12px 14px;
    transition: border-color var(--transition-fast), box-shadow var(--transition-fast);
  }

  .subtitle-editor-field textarea:focus {
    outline: none;
    border-color: var(--accent);
    box-shadow: 0 0 0 3px var(--accent-soft);
  }

  .subtitle-empty-state {
    min-height: 52vh;
    display: grid;
    place-items: center;
    gap: 12px;
    padding: 24px;
    text-align: center;
    border-radius: 20px;
    background: var(--bg-surface);
    border: 1px solid var(--border-subtle);
  }

  .subtitle-empty-state h2,
  .subtitle-empty-state p {
    margin: 0;
  }

  .subtitle-empty-state p {
    color: var(--text-secondary);
  }

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
  }

  @media (max-width: 980px) {
    .subtitle-header {
      flex-direction: column;
      align-items: stretch;
    }

    .subtitle-grid-head {
      display: none;
    }

    .subtitle-editor-row {
      grid-template-columns: 1fr;
    }

    .subtitle-editor-time {
      grid-auto-flow: column;
      justify-content: space-between;
      align-items: center;
      padding-top: 0;
    }
  }

  @media (max-width: 640px) {
    .subtitle-header-actions {
      width: 100%;
    }

    .subtitle-header-actions :global(button) {
      flex: 1;
    }

    .subtitle-editor-row {
      padding: 10px;
    }

    .subtitle-editor-field textarea {
      min-height: 84px;
    }
  }
</style>
