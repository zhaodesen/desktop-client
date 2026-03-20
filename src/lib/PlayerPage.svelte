<script lang="ts">
  import type { PlaybackSnapshot, PlaybackHistoryEntry, PlaylistMode } from "../shared/types";

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
    history: PlaybackHistoryEntry[];
    onTogglePlayback: () => void;
    onSeek: (ms: number) => void;
    onRateChange: (rate: number) => void;
    onPlaylistModeChange: (mode: PlaylistMode) => void;
    onRetryAsr: () => void;
    onPlayHistory: (id: string) => void;
  }

  const {
    snap, hasMedia, audioFileLabel, subtitleFileLabel, cueTiming,
    currentText, currentSecondaryText, playbackRate, playlistMode,
    showRetryAsr, history,
    onTogglePlayback, onSeek, onRateChange, onPlaylistModeChange,
    onRetryAsr, onPlayHistory,
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

  function formatTimestamp(ts: number): string {
    return new Date(ts).toLocaleString("zh-CN", {
      month: "2-digit", day: "2-digit", hour: "2-digit", minute: "2-digit",
    });
  }

  const dur = $derived(Math.max(snap.durationMs, 0));
  const progress = $derived(Math.min(snap.currentTimeMs, dur || snap.currentTimeMs));
</script>

<section class="page" data-active="true">
  <header class="page-header">
    <h2>播放器</h2>
  </header>

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

    <div class="player-controls">
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
          <span>{hasMedia ? `时长 ${formatDuration(dur)} · ${snap.rate.toFixed(2)}x` : "等待导入媒体"}</span>
        </div>
      </div>
    </div>

    <div class="player-options">
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
      {#if showRetryAsr}
        <button class="btn btn-outline btn-sm" type="button" title="重新生成字幕" onclick={onRetryAsr}>
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="23 4 23 10 17 10"/><path d="M20.49 15a9 9 0 1 1-2.12-9.36L23 10"/></svg>
          重新识别
        </button>
      {/if}
    </div>

    <div class="subtitle-preview">
      <span class="label-sm">当前字幕</span>
      <p class="subtitle-text">{currentText}</p>
      {#if currentSecondaryText}
        <p class="subtitle-secondary-text">{currentSecondaryText}</p>
      {/if}
    </div>
  </div>

  <div class="section-bar">
    <span class="section-title">播放历史</span>
    <span class="badge">{history.length}</span>
  </div>

  {#if history.length === 0}
    <div class="list empty-state">
      <div class="empty-content">
        <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" opacity="0.4"><circle cx="12" cy="12" r="10"/><polyline points="12 6 12 12 16 14"/></svg>
        <span>还没有播放记录</span>
      </div>
    </div>
  {:else}
    <div class="list">
      {#each history as entry (entry.mediaId)}
        <div class="list-item">
          <div class="list-item-info">
            <div class="list-item-title">{entry.title}</div>
            <div class="list-item-meta">
              <span>播放 {entry.playCount} 次</span>
              <span>{formatTimestamp(entry.playedAt)}</span>
              <span>{entry.subtitlePath ? "有字幕" : "无字幕"}</span>
            </div>
          </div>
          <div class="list-item-actions">
            <button class="btn btn-sm" onclick={() => onPlayHistory(entry.mediaId)}>播放</button>
          </div>
        </div>
      {/each}
    </div>
  {/if}
</section>
