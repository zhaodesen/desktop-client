<script lang="ts">
  import type { SubtitleDocument, SubtitleCue } from "../shared/types";

  interface Props {
    document: SubtitleDocument | undefined;
    lastMainPage: string;
    onBack: () => void;
    onSave: () => void;
    onCueChange: (index: number, field: "text" | "secondaryText", value: string) => void;
  }

  const { document, lastMainPage, onBack, onSave, onCueChange }: Props = $props();

  function formatDuration(ms: number): string {
    const t = Math.max(0, Math.floor(ms / 1000));
    const h = Math.floor(t / 3600);
    const m = Math.floor((t % 3600) / 60);
    const s = t % 60;
    const mm = String(m).padStart(2, "0");
    const ss = String(s).padStart(2, "0");
    return h > 0 ? `${String(h).padStart(2, "0")}:${mm}:${ss}` : `${mm}:${ss}`;
  }

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
      <button class="btn btn-primary" type="button" onclick={onSave}>保存</button>
    </div>
  </header>

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
              <!-- svelte-ignore a11y_label_has_associated_control -->
              <label>原文</label>
              <textarea
                value={cue.text}
                oninput={(e) => onCueChange(index, "text", (e.target as HTMLTextAreaElement).value)}
              ></textarea>
            </div>
            <div class="subtitle-editor-field">
              <!-- svelte-ignore a11y_label_has_associated_control -->
              <label>中文字幕</label>
              <textarea
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
