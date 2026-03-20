<script lang="ts">
  import type { MediaItem } from "../shared/types";

  interface Props {
    items: MediaItem[];
    onImportMedia: () => void;
    onImportSubtitle: () => void;
    onPlayMedia: (id: string) => void;
    onEditSubtitle: (id: string) => void;
    onDeleteMedia: (id: string) => void;
  }

  const { items, onImportMedia, onImportSubtitle, onPlayMedia, onEditSubtitle, onDeleteMedia }: Props = $props();

  function formatTimestamp(ts: number): string {
    return new Date(ts).toLocaleString("zh-CN", {
      month: "2-digit", day: "2-digit", hour: "2-digit", minute: "2-digit",
    });
  }
</script>

<section class="page" data-active="true">
  <header class="page-header">
    <h2>素材库</h2>
    <div class="header-actions">
      <button class="btn btn-outline" type="button" onclick={onImportSubtitle}>
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><polyline points="14 2 14 8 20 8"/><line x1="16" y1="13" x2="8" y2="13"/><line x1="16" y1="17" x2="8" y2="17"/></svg>
        导入字幕
      </button>
      <button class="btn btn-primary" type="button" onclick={onImportMedia}>
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>
        导入媒体
      </button>
    </div>
  </header>

  <div class="card info-banner">
    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><line x1="12" y1="16" x2="12" y2="12"/><line x1="12" y1="8" x2="12.01" y2="8"/></svg>
    <span>导入音频或视频后，自动转存本地、离线生成字幕，并补全中文字幕。</span>
  </div>

  <div class="section-bar">
    <span class="section-title">已导入</span>
    <span class="badge">{items.length}</span>
  </div>

  {#if items.length === 0}
    <div class="list empty-state">
      <div class="empty-content">
        <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" opacity="0.4"><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/></svg>
        <span>还没有导入任何素材</span>
      </div>
    </div>
  {:else}
    <div class="list">
      {#each items as item (item.id)}
        <div class="list-item">
          <div class="list-item-info">
            <div class="list-item-title">{item.title}</div>
            <div class="list-item-meta">
              <span>{item.sourceKind === "video" ? "视频" : "音频"}</span>
              <span>{item.subtitlePath ? "已生成字幕" : "待生成字幕"}</span>
              <span>{formatTimestamp(item.importedAt)}</span>
            </div>
          </div>
          <div class="list-item-actions">
            <button class="btn btn-sm" onclick={() => onPlayMedia(item.id)}>播放</button>
            <button class="btn btn-sm" disabled={!item.subtitlePath} onclick={() => onEditSubtitle(item.id)}>编辑字幕</button>
            <button class="btn btn-sm btn-danger" onclick={() => onDeleteMedia(item.id)}>删除</button>
          </div>
        </div>
      {/each}
    </div>
  {/if}
</section>
