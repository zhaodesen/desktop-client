<script lang="ts">
  import type { PlaybackSnapshot, PlaybackHistoryItem, PlaylistMode } from "../shared/types";

  interface Props {
    snap: PlaybackSnapshot;
    hasMedia: boolean;
    audioFileLabel: string;
    subtitleFileLabel: string;
    cueTiming: string;
    currentText: string;
    currentSecondaryText: string;
    playbackRate: number;
    playlistMode: PlaylistMode;
    showRetryAsr: boolean;
    playlist: PlaybackHistoryItem[];
    currentMediaId: string | undefined;
    onTogglePlayback: () => void;
    onSeek: (ms: number) => void;
    onRateChange: (rate: number) => void;
    onPlaylistModeChange: (mode: PlaylistMode) => void;
    onRetryAsr: () => void;
    onPlayItem: (id: string) => void;
  }

  const {
    snap, hasMedia, audioFileLabel, subtitleFileLabel, cueTiming,
    currentText, currentSecondaryText, playbackRate, playlistMode,
    showRetryAsr, playlist, currentMediaId,
    onTogglePlayback, onSeek, onRateChange, onPlaylistModeChange,
    onRetryAsr, onPlayItem,
  }: Props = $props();

  function formatDuration(ms: number): string {
    const t = Math.max(0, Math.floor(ms / 1000));
    const h = Math.floor(t / 3600);
    const m = Math.floor((t % 3600) / 60);
    const s = t % 60;
    const mm = String(m).padStart(2, "0");
    const ss = String(s).padStart(2, "0");
    return h > 0 ? `${String(h).padStart(2, "0")}:${mm}:${ss}` : `${mm}:${ss}`;
  }

  const dur = $derived(Math.max(snap.durationMs, 0));
  const progress = $derived(Math.min(snap.currentTimeMs, dur || snap.currentTimeMs));
</script>

<section class="page playlist-page" data-active="true">
  <!-- 字幕预览区 -->
  <div class="card player-card">
    <div class="player-now">
      <div class="player-now-info">
        <span class="label-sm">当前播放</span>
        <h3>{audioFileLabel}</h3>
      </div>
      <div class="player-now-meta">
        <span class="tag">{subtitleFileLabel}</span>
        <span class="tag tag-dim">{cueTiming}</span>
      </div>
    </div>

    <div class="subtitle-preview">
      <span class="label-sm">当前字幕</span>
      <p class="subtitle-text">{currentText}</p>
      {#if currentSecondaryText}
        <p class="subtitle-secondary-text">{currentSecondaryText}</p>
      {/if}
    </div>

    {#if showRetryAsr}
      <button class="btn btn-outline btn-sm" type="button" title="重新生成字幕" onclick={onRetryAsr}>
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="23 4 23 10 17 10"/><path d="M20.49 15a9 9 0 1 1-2.12-9.36L23 10"/></svg>
        重新识别
      </button>
    {/if}
  </div>

  <!-- 播放列表 -->
  <div class="section-bar">
    <span class="section-title">播放列表</span>
    <span class="badge">{playlist.length}</span>
  </div>

  {#if playlist.length === 0}
    <div class="list empty-state">
      <div class="empty-content">
        <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" opacity="0.4"><circle cx="12" cy="12" r="10"/><polyline points="12 6 12 12 16 14"/></svg>
        <span>播放列表为空，从资源列表添加</span>
      </div>
    </div>
  {:else}
    <div class="list playlist-list">
      {#each playlist as entry (entry.mediaId)}
        <button
          class="playlist-item"
          class:playlist-item-active={entry.mediaId === currentMediaId}
          onclick={() => onPlayItem(entry.mediaId)}
        >
          <div class="playlist-item-indicator">
            {#if entry.mediaId === currentMediaId && snap.playing}
              <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor"><rect x="6" y="4" width="4" height="16"/><rect x="14" y="4" width="4" height="16"/></svg>
            {:else if entry.mediaId === currentMediaId}
              <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor"><polygon points="5 3 19 12 5 21 5 3"/></svg>
            {:else}
              <span class="playlist-item-num">&bull;</span>
            {/if}
          </div>
          <div class="playlist-item-info">
            <span class="playlist-item-title">{entry.title}</span>
            <span class="playlist-item-meta">{entry.subtitlePath ? "有字幕" : "无字幕"}</span>
          </div>
        </button>
      {/each}
    </div>
  {/if}

  <!-- 底部固定播放控制栏 -->
  <div class="player-bar">
    <div class="player-bar-controls">
      <button
        class="btn btn-icon btn-primary"
        type="button"
        disabled={!hasMedia}
        title="播放/暂停 (空格)"
        onclick={onTogglePlayback}
      >
        {#if snap.playing}
          <svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor"><rect x="6" y="4" width="4" height="16"/><rect x="14" y="4" width="4" height="16"/></svg>
        {:else}
          <svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor"><polygon points="5 3 19 12 5 21 5 3"/></svg>
        {/if}
      </button>
      <div class="progress-wrap">
        <input
          type="range"
          min="0"
          max={Math.max(dur, 1)}
          value={progress}
          disabled={!hasMedia}
          oninput={(e) => onSeek(Number((e.target as HTMLInputElement).value))}
        />
        <div class="progress-time">
          <span>{formatDuration(snap.currentTimeMs)} / {formatDuration(dur)}</span>
          <span>{hasMedia ? `${snap.rate.toFixed(2)}x` : ""}</span>
        </div>
      </div>
    </div>
    <div class="player-bar-options">
      <label class="inline-field">
        <span>倍率</span>
        <select value={playbackRate.toFixed(2)} onchange={(e) => onRateChange(Number((e.target as HTMLSelectElement).value))}>
          <option value="0.50">0.50x</option>
          <option value="0.75">0.75x</option>
          <option value="0.80">0.80x</option>
          <option value="1.00">1.00x</option>
          <option value="1.25">1.25x</option>
          <option value="1.50">1.50x</option>
          <option value="2.00">2.00x</option>
        </select>
      </label>
      <label class="inline-field">
        <span>循环</span>
        <select value={playlistMode} onchange={(e) => onPlaylistModeChange((e.target as HTMLSelectElement).value as PlaylistMode)}>
          <option value="sequential">顺序播放</option>
          <option value="single">单曲循环</option>
        </select>
      </label>
    </div>
  </div>
</section>

<style>
  .playlist-page {
    display: flex;
    flex-direction: column;
    gap: 16px;
    padding-bottom: 140px; /* space for bottom bar */
  }

  .playlist-list {
    gap: 2px;
  }

  .playlist-item {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 14px;
    border: none;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--text-primary);
    font: inherit;
    font-size: 0.88rem;
    cursor: pointer;
    text-align: left;
    width: 100%;
    transition: background 150ms;
  }

  .playlist-item:hover {
    background: var(--bg-surface);
  }

  .playlist-item-active {
    background: var(--accent-soft);
    color: var(--accent);
  }
  .playlist-item-active:hover {
    background: var(--accent-soft);
  }

  .playlist-item-indicator {
    width: 20px;
    display: flex;
    justify-content: center;
    flex-shrink: 0;
    color: var(--accent);
  }

  .playlist-item-num {
    color: var(--text-dim);
  }

  .playlist-item-info {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .playlist-item-title {
    font-weight: 500;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .playlist-item-meta {
    font-size: 0.72rem;
    color: var(--text-dim);
  }

  /* 底部固定播放控制栏 */
  .player-bar {
    position: fixed;
    bottom: 0;
    right: 0;
    left: var(--sidebar-w);
    background: var(--bg-raised);
    border-top: 1px solid var(--border);
    padding: 14px 28px;
    display: flex;
    flex-direction: column;
    gap: 10px;
    z-index: 10;
  }

  .player-bar-controls {
    display: flex;
    align-items: center;
    gap: 14px;
  }

  .player-bar-options {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  @media (max-width: 900px) {
    .player-bar {
      left: 0;
    }
  }
</style>
