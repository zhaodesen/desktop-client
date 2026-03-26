<script lang="ts">
  import { onDestroy } from "svelte";
  import { convertFileSrc } from "@tauri-apps/api/core";
  import type { MediaItem } from "../shared/types";

  interface Props {
    items: MediaItem[];
    onRetryAsr: (id: string) => void;
    onEditSubtitle: (id: string) => void;
    onDeleteMedia: (id: string) => void;
    onAddToPlaylist: (id: string) => void;
  }

  const { items, onRetryAsr, onEditSubtitle, onDeleteMedia, onAddToPlaylist }: Props = $props();
  let durationLabels = $state<Record<string, string>>({});
  let addedTooltipId = $state<string | undefined>(undefined);
  let tooltipTimer: ReturnType<typeof setTimeout> | undefined;
  const pendingDurationIds = new Set<string>();

  function formatTimestamp(ts: number): string {
    return new Date(ts).toLocaleString("zh-CN", {
      month: "2-digit", day: "2-digit", hour: "2-digit", minute: "2-digit",
    });
  }

  function formatDuration(ms: number): string {
    const totalSeconds = Math.max(0, Math.round(ms / 1000));
    const hours = Math.floor(totalSeconds / 3600);
    const minutes = Math.floor((totalSeconds % 3600) / 60);
    const seconds = totalSeconds % 60;
    if (hours > 0) {
      return `${String(hours).padStart(2, "0")}:${String(minutes).padStart(2, "0")}:${String(seconds).padStart(2, "0")}`;
    }
    return `${String(minutes).padStart(2, "0")}:${String(seconds).padStart(2, "0")}`;
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
  <header class="page-header">
    <h2>资源列表</h2>
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
              <span>{durationLabels[item.id] ?? "读取时长中…"}</span>
              <span>{item.subtitlePath ? "已生成字幕" : "待生成字幕"}</span>
              <span>{formatTimestamp(item.importedAt)}</span>
            </div>
          </div>
          <div class="list-item-actions">
            <div class="add-btn-wrap">
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
            <button class="btn btn-sm" onclick={() => onRetryAsr(item.id)}>重新识别</button>
            <button class="btn btn-sm" disabled={!item.subtitlePath} onclick={() => onEditSubtitle(item.id)}>编辑字幕</button>
            <button class="btn btn-sm btn-danger" onclick={() => onDeleteMedia(item.id)}>删除</button>
          </div>
        </div>
      {/each}
    </div>
  {/if}
</section>

<style>
  /* btn-icon-sm is now in styles.css */

  .add-btn-wrap {
    position: relative;
    display: inline-flex;
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

  .add-tooltip {
    position: absolute;
    top: calc(100% + 8px);
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
    animation: tooltip-in 0.15s ease;
  }

  @keyframes tooltip-in {
    from { opacity: 0; transform: translateX(-50%) translateY(-4px); }
    to   { opacity: 1; transform: translateX(-50%) translateY(0); }
  }
</style>
