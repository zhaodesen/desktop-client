<script lang="ts">
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
  let searchQuery = $state("");
  const pendingDurationIds = new Set<string>();

  const normalizedSearchQuery = $derived(searchQuery.trim().toLocaleLowerCase("zh-CN"));
  const filteredItems = $derived.by(() => {
    if (!normalizedSearchQuery) return items;
    return items.filter((item) => item.title.toLocaleLowerCase("zh-CN").includes(normalizedSearchQuery));
  });

  const subtitledCount = $derived(items.filter((item) => Boolean(item.subtitlePath)).length);
  const filteredCount = $derived(filteredItems.length);
  const pendingSubtitleCount = $derived(items.length - subtitledCount);

  function formatTimestamp(ts: number): string {
    return new Date(ts).toLocaleString("zh-CN", {
      year: "numeric",
      month: "2-digit",
      day: "2-digit",
      hour: "2-digit",
      minute: "2-digit",
      second: "2-digit",
      hour12: false,
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
  }

  $effect(() => {
    for (const item of items) {
      void resolveDuration(item);
    }
  });
</script>

<section class="page resource-page" data-active="true">
  {#if retryCompletedMediaId}
    <div class="retry-complete-notice" role="status">
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <path d="M20 6 9 17l-5-5"/>
      </svg>
      <span>{retryCompletedMessage ?? "重新识别完成"}</span>
    </div>
  {/if}

  <header class="page-header resource-header">
    <div class="resource-header-copy">
      <h2>资源列表</h2>
      <p>管理已导入的音频与视频。</p>
    </div>
  </header>

  <section class="resource-toolbar" aria-label="资源筛选与概览">
    <div class="toolbar-search-row">
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
    </div>
  </section>
  {#if items.length === 0}
    <div class="resource-list-scroll list empty-state">
      <div class="empty-content resource-empty">
        <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" opacity="0.4"><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/></svg>
        <strong>片库还是空的</strong>
        <span>先去导入页面加入音频或视频，资源列表会按播放器的方式整理它们。</span>
      </div>
    </div>
  {:else if filteredItems.length === 0}
    <div class="resource-list-scroll list empty-state">
      <div class="empty-content resource-empty">
        <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" opacity="0.4"><circle cx="11" cy="11" r="7"></circle><line x1="21" y1="21" x2="16.65" y2="16.65"></line></svg>
        <strong>没有匹配结果</strong>
        <span>当前关键词“{searchQuery}”没有命中任何资源，换个片名或清空搜索试试。</span>
      </div>
    </div>
  {:else}
    <div class="resource-list-scroll resource-library-scroll">
      <Virtualizer data={filteredItems} overscan={6} itemSize={88} getKey={(item) => item.id}>
        {#snippet children(item, index)}
          <article
            class="list-item library-row"
            class:list-item-retrying={item.id === retryingMediaId}
            class:list-item-retry-done={item.id === retryCompletedMediaId}
          >
            {#if !item.subtitlePath}
              <span class="library-pending-badge">待识别</span>
            {/if}
            {#if item.id === retryingMediaId}
              <div
                class="list-item-progress-bg"
                aria-hidden="true"
                style={`width: ${Math.max(retryingProgress, 6)}%;`}
              ></div>
            {/if}

            <div class="library-main">
              <div class="library-index-pill" aria-hidden="true">{String(index + 1).padStart(2, "0")}</div>

              <div class="list-item-info library-copy">
                <div class="library-title-row">
                  <div class="list-item-title library-title">{item.title}</div>
                </div>

                <div class="list-item-meta library-meta">
                  <span>时长 {durationLabels[item.id] ?? "读取中"}</span>
                  <span>导入 {formatTimestamp(item.importedAt)}</span>
                </div>

                {#if item.id === retryingMediaId}
                  <div class="retry-asr-status" role="status">
                    <span class="retry-asr-status-title">重新识别中 {Math.round(retryingProgress)}%</span>
                    <span class="retry-asr-status-message">{retryingMessage ?? "正在后台处理…"}</span>
                  </div>
                {/if}
              </div>
            </div>

            <div class="library-side">
              <div class="list-item-actions library-actions">
                <button
                  class="btn btn-sm btn-icon-sm btn-primary-soft"
                  title="播放并加入播放列表"
                  onclick={() => handleAdd(item.id)}
                >
                  <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
                    <polygon points="8 5 19 12 8 19 8 5"/>
                  </svg>
                </button>

                <button
                  class="btn btn-sm btn-ghost"
                  disabled={asrBusy}
                  onclick={() => onRetryAsr(item.id)}
                >
                  {item.id === retryingMediaId ? "识别中…" : "重新识别"}
                </button>
                <button class="btn btn-sm" disabled={!item.subtitlePath} onclick={() => onEditSubtitle(item.id)}>编辑字幕</button>
                <button class="btn btn-sm btn-danger" onclick={() => onDeleteMedia(item.id)}>删除</button>
              </div>
            </div>
          </article>
        {/snippet}
      </Virtualizer>
    </div>
  {/if}
</section>

<style>
  section.page.resource-page {
    flex: 1;
    min-height: 0;
    overflow: hidden;
    gap: 12px;
  }

  .resource-header {
    display: flex;
    justify-content: space-between;
    align-items: flex-end;
    gap: 16px;
  }

  .resource-header-copy {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .resource-header-copy p {
    font-size: var(--font-xs);
    color: var(--text-dim);
  }

  .resource-toolbar {
    display: block;
  }

  .toolbar-search-row {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 8px;
    align-items: center;
  }

  .resource-search {
    display: flex;
    align-items: center;
    gap: 10px;
    min-height: 40px;
    padding: 0 12px;
    border: 1px solid var(--border);
    border-radius: 12px;
    background: transparent;
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

  .resource-list-scroll {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    overflow-x: hidden;
    scrollbar-gutter: stable;
  }

  .resource-library-scroll {
    padding-right: 2px;
  }

  .library-row {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    align-items: center;
    gap: 12px;
    min-height: 74px;
    padding: 8px 10px;
    border-radius: 12px;
    background: transparent;
    border: 1px solid var(--border-subtle);
  }

  .library-main,
  .library-side {
    position: relative;
    z-index: 1;
  }

  .library-main {
    display: grid;
    grid-template-columns: 34px minmax(0, 1fr);
    gap: 10px;
    align-items: center;
    min-width: 0;
  }

  .library-pending-badge {
    position: absolute;
    top: 6px;
    left: 6px;
    display: inline-flex;
    align-items: center;
    height: 16px;
    padding: 0 5px;
    border-radius: 999px;
    font-size: 9px;
    line-height: 1;
    font-weight: 700;
    letter-spacing: 0.02em;
    color: rgba(255, 255, 255, 0.72);
    background: rgba(255, 255, 255, 0.12);
    border: 1px solid rgba(255, 255, 255, 0.08);
    backdrop-filter: blur(6px);
    -webkit-backdrop-filter: blur(6px);
    z-index: 2;
  }

  .library-index-pill {
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 11px;
    line-height: 1;
    font-weight: 700;
    color: var(--text-secondary);
    font-family: "SF Mono", "JetBrains Mono", "IBM Plex Mono", "Roboto Mono", ui-monospace, monospace;
    font-variant-numeric: tabular-nums;
    letter-spacing: -0.01em;
  }

  .library-copy {
    gap: 4px;
  }

  .library-title-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    min-width: 0;
  }

  .library-title {
    font-size: var(--font-base);
    font-weight: 700;
    letter-spacing: -0.01em;
  }

  .library-meta {
    gap: 4px;
    font-size: 11px;
  }

  .library-meta span {
    max-width: 100%;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .library-side {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 8px;
    min-width: 0;
  }

  .library-actions {
    justify-content: flex-end;
  }

  .btn-primary-soft {
    background: transparent;
    border-color: var(--border);
    color: var(--accent);
  }

  .btn-primary-soft:hover {
    background: var(--accent-soft);
    border-color: var(--accent-border);
  }

  .resource-empty {
    gap: 8px;
    text-align: center;
  }

  .resource-empty strong {
    font-size: var(--font-base);
    color: var(--text-primary);
  }

  .resource-empty span {
    max-width: 32ch;
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
      rgba(var(--accent-rgb), 0.08) 100%
    );
    pointer-events: none;
    transition: width 180ms ease;
  }

  .retry-asr-status {
    display: flex;
    align-items: baseline;
    gap: 10px;
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

  @media (max-width: 1120px) {
    .resource-header {
      align-items: flex-start;
      flex-direction: column;
    }

    .library-row {
      grid-template-columns: 1fr;
      gap: 10px;
    }
  }

  @media (max-width: 760px) {
    section.page.resource-page {
      gap: 10px;
    }

    .toolbar-search-row {
      grid-template-columns: 1fr;
    }

    .library-title-row {
      flex-direction: column;
      align-items: flex-start;
    }

    .library-side {
      justify-content: flex-start;
    }

    .library-actions {
      flex-wrap: wrap;
      justify-content: flex-start;
    }
  }
</style>
