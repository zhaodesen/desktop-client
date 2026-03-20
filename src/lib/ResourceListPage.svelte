<script lang="ts">
  import type { MediaItem } from "../shared/types";

  interface Props {
    items: MediaItem[];
    onEditSubtitle: (id: string) => void;
    onDeleteMedia: (id: string) => void;
    onAddToPlaylist: (id: string) => void;
    onImportMedia: () => void;
  }

  const { items, onEditSubtitle, onDeleteMedia, onAddToPlaylist, onImportMedia }: Props = $props();

  function formatTimestamp(ts: number): string {
    return new Date(ts).toLocaleString("zh-CN", {
      month: "2-digit", day: "2-digit", hour: "2-digit", minute: "2-digit",
    });
  }
</script>

<section class="page" data-active="true">
  <header class="page-header">
    <h2>资源列表</h2>
    <div class="header-actions">
      <button class="btn btn-primary" type="button" onclick={onImportMedia}>
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>
        导入
      </button>
    </div>
  </header>

  <div class="section-bar">
    <span class="section-title">已导入</span>
    <span class="badge">{items.length}</span>
  </div>

  {#if items.length === 0}
    <div class="list empty-state">
      <div class="empty-content">
        <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" opacity="0.4"><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/></svg>
        <span>还没有导入任何资源，前往导入页面添加</span>
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
            <button
              class="btn btn-sm btn-icon-sm"
              title="加入播放列表"
              onclick={() => onAddToPlaylist(item.id)}
            >
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>
            </button>
            <button class="btn btn-sm" disabled={!item.subtitlePath} onclick={() => onEditSubtitle(item.id)}>编辑字幕</button>
            <button class="btn btn-sm btn-danger" onclick={() => onDeleteMedia(item.id)}>删除</button>
          </div>
        </div>
      {/each}
    </div>
  {/if}
</section>

<style>
  :global(.btn-icon-sm) {
    width: 30px;
    height: 30px;
    padding: 0;
    justify-content: center;
    border-radius: 50%;
    flex-shrink: 0;
  }
</style>
