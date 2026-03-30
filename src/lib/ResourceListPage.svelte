<script lang="ts">
  import { onDestroy } from "svelte";
  import { Virtualizer } from "virtua/svelte";
  import { convertFileSrc } from "@tauri-apps/api/core";
  import type { MediaItem } from "../shared/types";
  import { formatDuration } from "../shared/utils";

  interface Props {
    items: MediaItem[];
    retryingMediaId?: string;
    retryingProgress?: number;
    retryingMessage?: string;
    retryCompletedMediaId?: string;
    retryCompletedMessage?: string;
    asrBusy?: boolean;
    onRetryAsr: (id: string) => void;
    onEditSubtitle: (id: string) => void;
    onDeleteMedia: (id: string) => void;
    onAddToPlaylist: (id: string) => void;
  }

  const {
    items,
    retryingMediaId,
    retryingProgress = 0,
    retryingMessage,
    retryCompletedMediaId,
    retryCompletedMessage,
    asrBusy = false,
    onRetryAsr,
    onEditSubtitle,
    onDeleteMedia,
    onAddToPlaylist,
  }: Props = $props();
  let durationLabels = $state<Record<string, string>>({});
  let addedTooltipId = $state<string | undefined>(undefined);
  let searchQuery = $state("");
  let tooltipTimer: ReturnType<typeof setTimeout> | undefined;
  const pendingDurationIds = new Set<string>();
  const normalizedSearchQuery = $derived(searchQuery.trim().toLocaleLowerCase("zh-CN"));
  const filteredItems = $derived.by(() => {
    if (!normalizedSearchQuery) return items;
    return items.filter((item) => item.title.toLocaleLowerCase("zh-CN").includes(normalizedSearchQuery));
  });

  function formatTimestamp(ts: number): string {
    return new Date(ts).toLocaleString("zh-CN", {
      month: "2-digit", day: "2-digit", hour: "2-digit", minute: "2-digit",
    });
  }

  async function resolveDuration(item: MediaItem) {
    if (durationLabels[item.id] || pendingDurationIds.has(item.id)) return;
    pendingDurationIds.add(item.id);
    try {
      const audio = new Audio();
      audio.preload = "metadata";
      audio.src = convertFileSrc(item.audioPath);
      const duration = await new Promise<number>((resolve, reject) => {
        const cleanup = () => {
          audio.removeEventListener("loadedmetadata", onLoaded);
          audio.removeEventListener("error", onError);
        };
        const onLoaded = () => {
          cleanup();
          resolve(Number.isFinite(audio.duration) ? audio.duration * 1000 : 0);
        };
        const onError = () => {
          cleanup();
          reject(new Error("媒体时长读取失败"));
        };
        audio.addEventListener("loadedmetadata", onLoaded);
        audio.addEventListener("error", onError);
      });
      durationLabels = { ...durationLabels, [item.id]: formatDuration(duration) };
      audio.src = "";
    } catch {
      durationLabels = { ...durationLabels, [item.id]: "--:--" };
    } finally {
      pendingDurationIds.delete(item.id);
    }
  }

  function handleAdd(itemId: string) {
    onAddToPlaylist(itemId);
    addedTooltipId = itemId;
    clearTimeout(tooltipTimer);
    tooltipTimer = setTimeout(() => {
      if (addedTooltipId === itemId) {
        addedTooltipId = undefined;
      }
    }, 1300);
  }

  $effect(() => {
    for (const item of items) {
      void resolveDuration(item);
    }
  });

  onDestroy(() => {
    clearTimeout(tooltipTimer);
  });
</script>

<section class="page" data-active="true">
  {#if retryCompletedMediaId}
    <div class="retry-complete-notice" role="status">
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <path d="M20 6 9 17l-5-5"/>
      </svg>
      <span>{retryCompletedMessage ?? "重新识别完成"}</span>
    </div>
  {/if}

  <header class="page-header">
    <h2>资源列表</h2>
  </header>

  <div class="section-bar">
    <span class="section-title">已导入</span>
    <span class="badge">{items.length}</span>
  </div>

  <label class="resource-search" for="resource-search-input">
    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
      <circle cx="11" cy="11" r="7"></circle>
      <line x1="21" y1="21" x2="16.65" y2="16.65"></line>
    </svg>
    <input
      id="resource-search-input"
      type="text"
      placeholder="按资源名称搜索"
      bind:value={searchQuery}
    />
    {#if searchQuery}
      <button
        class="resource-search-clear"
        type="button"
        title="清空搜索"
        onclick={() => { searchQuery = ""; }}
      >
        <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
          <line x1="18" y1="6" x2="6" y2="18"></line>
          <line x1="6" y1="6" x2="18" y2="18"></line>
        </svg>
      </button>
    {/if}
  </label>

  {#if items.length === 0}
    <div class="resource-list-scroll list empty-state">
      <div class="empty-content">
        <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" opacity="0.4"><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/></svg>
        <span>还没有导入任何资源，前往导入页面添加</span>
      </div>
    </div>
  {:else if filteredItems.length === 0}
    <div class="resource-list-scroll list empty-state">
      <div class="empty-content">
        <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" opacity="0.4"><circle cx="11" cy="11" r="7"></circle><line x1="21" y1="21" x2="16.65" y2="16.65"></line></svg>
        <span>没有找到匹配“{searchQuery}”的资源</span>
      </div>
    </div>
  {:else}
    <div class="resource-list-scroll">
      <Virtualizer data={filteredItems} overscan={6}>
        {#snippet children(item)}
          <div
            class="list-item"
            class:list-item-retrying={item.id === retryingMediaId}
            class:list-item-retry-done={item.id === retryCompletedMediaId}
          >
            {#if item.id === retryingMediaId}
              <div
                class="list-item-progress-bg"
                aria-hidden="true"
                style={`width: ${Math.max(retryingProgress, 6)}%;`}
              ></div>
            {/if}
            <div class="list-item-info">
              <div class="list-item-title">{item.title}</div>
              <div class="list-item-meta">
                <span>{item.sourceKind === "video" ? "视频" : "音频"}</span>
                <span>{durationLabels[item.id] ?? "读取时长中…"}</span>
                <span>{item.subtitlePath ? "已生成字幕" : "待生成字幕"}</span>
                <span>{formatTimestamp(item.importedAt)}</span>
              </div>
              {#if item.id === retryingMediaId}
                <div class="retry-asr-status" role="status">
                  <span class="retry-asr-status-title">重新识别中 {Math.round(retryingProgress)}%</span>
                  <span class="retry-asr-status-message">{retryingMessage ?? "正在后台处理…"}</span>
                </div>
              {/if}
            </div>
            <div class="list-item-actions">
              <div class="add-btn-wrap" class:add-btn-wrap-active={addedTooltipId === item.id}>
                <button
                  class="btn btn-sm btn-icon-sm"
                  title="加入播放列表"
                  onclick={() => handleAdd(item.id)}
                >
                  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>
                </button>
                {#if addedTooltipId === item.id}
                  <span class="add-tooltip">已添加到播放列表</span>
                {/if}
              </div>
              <button
                class="btn btn-sm"
                disabled={asrBusy}
                onclick={() => onRetryAsr(item.id)}
              >
                {item.id === retryingMediaId ? "识别中…" : "重新识别"}
              </button>
              <button class="btn btn-sm" disabled={!item.subtitlePath} onclick={() => onEditSubtitle(item.id)}>编辑字幕</button>
              <button class="btn btn-sm btn-danger" onclick={() => onDeleteMedia(item.id)}>删除</button>
            </div>
          </div>
        {/snippet}
      </Virtualizer>
    </div>
  {/if}
</section>

<style>
  section.page {
    flex: 1;
    min-height: 0;
    overflow: hidden;
  }

  .resource-list-scroll {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    overflow-x: hidden;
    scrollbar-gutter: stable;
  }

  .resource-search {
    display: flex;
    align-items: center;
    gap: 10px;
    min-height: 42px;
    padding: 0 14px;
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    background: var(--bg-surface);
    color: var(--text-dim);
  }

  .resource-search:focus-within {
    border-color: var(--accent);
    box-shadow: 0 0 0 3px var(--accent-soft);
  }

  .resource-search input {
    flex: 1;
    min-width: 0;
    border: none;
    background: transparent;
    color: var(--text-primary);
    font: inherit;
    padding: 0;
    box-shadow: none;
  }

  .resource-search input::placeholder {
    color: var(--text-dim);
  }

  .resource-search input:focus {
    outline: none;
  }

  .resource-search-clear {
    display: grid;
    place-items: center;
    width: 22px;
    height: 22px;
    padding: 0;
    border: none;
    border-radius: 999px;
    background: transparent;
    color: var(--text-dim);
    cursor: pointer;
    flex-shrink: 0;
  }

  .resource-search-clear:hover {
    background: var(--bg-surface-hover);
    color: var(--text-primary);
  }

  /* btn-icon-sm is now in styles.css */

  .add-btn-wrap {
    position: relative;
    display: inline-flex;
  }

  .add-btn-wrap-active {
    z-index: var(--z-toast, 100);
  }

  @media (hover: hover) and (pointer: fine) {
    .list-item-actions {
      opacity: 0;
      transform: translateX(6px);
      pointer-events: none;
      transition: opacity var(--transition-normal, 180ms ease), transform var(--transition-normal, 180ms ease);
    }

    .list-item:hover .list-item-actions,
    .list-item:focus-within .list-item-actions {
      opacity: 1;
      transform: translateX(0);
      pointer-events: auto;
    }
  }

  .list-item-retrying,
  .list-item-retry-done {
    overflow: hidden;
  }

  .list-item-progress-bg {
    position: absolute;
    left: 0;
    top: 0;
    bottom: 0;
    border-radius: inherit;
    background: linear-gradient(
      90deg,
      rgba(var(--accent-rgb), 0.18) 0%,
      rgba(var(--accent-rgb), 0.1) 100%
    );
    pointer-events: none;
    transition: width 180ms ease;
  }

  .list-item-info,
  .list-item-actions {
    position: relative;
    z-index: 1;
  }

  .retry-asr-status {
    display: flex;
    align-items: baseline;
    gap: 10px;
    margin-top: 8px;
    font-size: var(--font-xs);
    color: var(--accent);
    flex-wrap: wrap;
  }

  .retry-asr-status-title {
    font-weight: 700;
  }

  .retry-asr-status-message {
    color: var(--text-secondary);
  }

  .retry-complete-notice {
    position: fixed;
    top: 64px;
    left: calc(var(--sidebar-w) + ((100vw - var(--sidebar-w)) / 2));
    transform: translateX(-50%);
    z-index: var(--z-toast);
    display: inline-flex;
    align-items: center;
    gap: 8px;
    padding: 9px 14px;
    max-width: min(520px, calc(100vw - var(--sidebar-w) - 48px));
    border-radius: var(--radius-pill);
    background: var(--success-soft);
    color: var(--success);
    border: 1px solid rgba(52, 211, 153, 0.18);
    box-shadow: var(--shadow-md);
    backdrop-filter: blur(8px);
    -webkit-backdrop-filter: blur(8px);
    font-size: var(--font-xs);
    font-weight: 700;
    animation: retry-complete-notice-in 160ms ease;
  }

  @keyframes retry-complete-notice-in {
    from { opacity: 0; transform: translateX(-50%) translateY(-4px); }
    to { opacity: 1; transform: translateX(-50%) translateY(0); }
  }

  .add-tooltip {
    position: absolute;
    bottom: calc(100% + 8px);
    left: 50%;
    transform: translateX(-50%);
    white-space: nowrap;
    padding: 6px 12px;
    border-radius: var(--radius-pill);
    background: var(--bg-glass, var(--bg-raised));
    backdrop-filter: blur(8px);
    -webkit-backdrop-filter: blur(8px);
    border: 1px solid var(--border);
    color: var(--success);
    font-size: var(--font-2xs);
    line-height: 1;
    box-shadow: var(--shadow-md);
    pointer-events: none;
    z-index: var(--z-toast, 100);
    animation: tooltip-in 0.15s ease;
  }

  @keyframes tooltip-in {
    from { opacity: 0; transform: translateX(-50%) translateY(4px); }
    to   { opacity: 1; transform: translateX(-50%) translateY(0); }
  }
</style>
