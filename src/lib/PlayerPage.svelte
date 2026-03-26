<script lang="ts">
  import type { PlaybackSnapshot, PlaybackHistoryItem, PlaylistMode } from "../shared/types";
  import { formatDuration } from "../shared/utils";

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
    playlist: PlaybackHistoryItem[];
    currentMediaId: string | undefined;
    volume: number;
    onTogglePlayback: () => void;
    onToggleCurrentItem: () => void;
    onSeek: (ms: number) => void;
    onRateChange: (rate: number) => void;
    onPlaylistModeChange: (mode: PlaylistMode) => void;
    onPlayItem: (id: string) => void;
    onPlayItemNow: (id: string) => void;
    onRemoveItem: (id: string) => void;
    onVolumeChange: (volume: number) => void;
    onVolumeCommit: () => void;
  }

  const {
    snap, hasMedia, audioFileLabel, subtitleFileLabel, cueTiming,
    currentText, currentSecondaryText, playbackRate, playlistMode,
    playlist, currentMediaId, volume,
    onTogglePlayback, onToggleCurrentItem, onSeek, onRateChange, onPlaylistModeChange,
    onPlayItem, onPlayItemNow, onRemoveItem, onVolumeChange, onVolumeCommit,
  }: Props = $props();

  const dur = $derived(Math.max(snap.durationMs, 0));
  const progress = $derived(Math.min(snap.currentTimeMs, dur || snap.currentTimeMs));
  const progressPercent = $derived(dur > 0 ? (progress / dur) * 100 : 0);
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
        <div class="playlist-row">
          <button
            class="playlist-item"
            class:playlist-item-active={entry.mediaId === currentMediaId}
            onclick={() => onPlayItem(entry.mediaId)}
            ondblclick={() => {
              if (entry.mediaId === currentMediaId) onToggleCurrentItem();
              else onPlayItemNow(entry.mediaId);
            }}
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
          <button
            class="playlist-remove-btn"
            type="button"
            title="从播放列表移除"
            onclick={() => onRemoveItem(entry.mediaId)}
          >
            <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
          </button>
        </div>
      {/each}
    </div>
  {/if}

  <!-- 底部固定播放控制栏 -->
  <div class="player-bar">
    <div class="player-bar-progress">
      <div class="player-bar-progress-fill" style="width: {progressPercent}%"></div>
    </div>
    <div class="player-bar-inner">
      <div class="player-bar-controls">
        <button
          class="btn btn-icon btn-primary player-play-btn"
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
        <label class="inline-field inline-field-volume">
          <span>音量</span>
          <div class="volume-control">
            <input
              type="range"
              min="0"
              max="1"
              step="0.01"
              value={volume}
              oninput={(e) => onVolumeChange(Number((e.target as HTMLInputElement).value))}
              onchange={onVolumeCommit}
            />
            <strong>{Math.round(volume * 100)}%</strong>
          </div>
        </label>
      </div>
    </div>
  </div>
</section>

<style>
  .playlist-page {
    display: flex;
    flex-direction: column;
    gap: 18px;
    padding-bottom: 150px; /* space for bottom bar */
  }

  .playlist-list {
    gap: 2px;
  }

  .playlist-row {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 8px;
    align-items: center;
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
    font-size: var(--font-base);
    cursor: pointer;
    text-align: left;
    width: 100%;
    transition: background 150ms;
    min-width: 0;
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
    font-size: var(--font-2xs);
    color: var(--text-dim);
  }

  .playlist-remove-btn {
    display: grid;
    place-items: center;
    width: 34px;
    height: 34px;
    border: 1px solid transparent;
    background: transparent;
    color: var(--text-dim);
    border-radius: var(--radius-sm);
    cursor: pointer;
    opacity: 0;
    transition: opacity 150ms, border-color 150ms, color 150ms, background 150ms;
  }

  .playlist-row:hover .playlist-remove-btn {
    opacity: 1;
  }

  .playlist-remove-btn:hover {
    border-color: var(--danger-border);
    color: var(--danger);
    background: var(--danger-soft);
  }

  /* 底部固定播放控制栏 */
  .player-bar {
    position: fixed;
    bottom: 0;
    right: 0;
    left: var(--sidebar-w);
    background: var(--bg-glass);
    backdrop-filter: blur(16px) saturate(1.6);
    -webkit-backdrop-filter: blur(16px) saturate(1.6);
    border-top: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    z-index: var(--z-sticky);
  }

  /* Top progress strip with glow effect */
  .player-bar-progress {
    height: 3px;
    background: var(--bg-inset);
    position: relative;
    overflow: hidden;
  }

  .player-bar-progress-fill {
    position: absolute;
    inset: 0 auto 0 0;
    background: var(--accent);
    transition: width 0.3s ease;
    border-radius: 0 1.5px 1.5px 0;
    box-shadow: 0 0 8px rgba(var(--accent-rgb), 0.4), 0 0 2px rgba(var(--accent-rgb), 0.6);
  }

  /* Glowing dot at the leading edge of progress */
  .player-bar-progress-fill::after {
    content: "";
    position: absolute;
    right: -2px;
    top: 50%;
    transform: translateY(-50%);
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--accent);
    box-shadow: 0 0 6px 2px rgba(var(--accent-rgb), 0.5);
  }

  .player-bar-inner {
    padding: 14px 28px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .player-bar-controls {
    display: flex;
    align-items: center;
    gap: 14px;
  }

  /* Play button with subtle pulse when playing */
  .player-play-btn {
    box-shadow: 0 2px 12px rgba(var(--accent-rgb), 0.2);
    transition: background var(--transition-fast), box-shadow var(--transition-fast), transform 80ms;
    position: relative;
  }

  .player-play-btn:hover {
    box-shadow: 0 4px 20px rgba(var(--accent-rgb), 0.35);
  }

  /* Seekbar (range input) enhanced styling */
  .player-bar-controls .progress-wrap input[type="range"] {
    height: 5px;
    background: var(--bg-surface-hover);
    border-radius: 2.5px;
    position: relative;
  }

  .player-bar-controls .progress-wrap input[type="range"]::-webkit-slider-thumb {
    width: 14px;
    height: 14px;
    background: var(--accent);
    border: 2px solid var(--bg-base);
    box-shadow: 0 0 0 0 rgba(var(--accent-rgb), 0), 0 1px 4px rgba(0, 0, 0, 0.3);
    transition: transform 120ms ease, box-shadow 120ms ease;
  }

  .player-bar-controls .progress-wrap input[type="range"]:hover::-webkit-slider-thumb {
    transform: scale(1.3);
    box-shadow: 0 0 0 4px var(--accent-soft), 0 2px 8px rgba(0, 0, 0, 0.3);
  }

  .player-bar-controls .progress-wrap input[type="range"]:active::-webkit-slider-thumb {
    transform: scale(1.15);
    box-shadow: 0 0 0 6px var(--accent-soft), 0 2px 8px rgba(0, 0, 0, 0.3);
  }

  .player-bar-options {
    display: flex;
    align-items: center;
    gap: 12px;
    flex-wrap: wrap;
  }

  .inline-field-volume {
    min-width: 220px;
  }

  .volume-control {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .volume-control input[type="range"] {
    flex: 1;
    min-width: 120px;
  }

  @media (max-width: 900px) {
    .player-bar {
      left: 0;
    }
  }
</style>
