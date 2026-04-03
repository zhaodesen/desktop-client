<script lang="ts">
  import type { PlaybackSnapshot, PlaylistMode } from "../shared/types";
  import { formatDuration } from "../shared/utils";

  interface Props {
    snap: PlaybackSnapshot;
    hasMedia: boolean;
    audioFileLabel: string;
    subtitleFileLabel: string;
    cueTiming: string;
    overlayVisible: boolean;
    playbackRate: number;
    playlistMode: PlaylistMode;
    volume: number;
    onTogglePlayback: () => void;
    onSeek: (ms: number) => void;
    onRateChange: (rate: number) => void;
    onPlaylistModeChange: (mode: PlaylistMode) => void;
    onToggleMute: () => void;
    onToggleOverlayVisible: () => void;
    onVolumeChange: (volume: number) => void;
    onVolumeCommit: () => void;
    onPrevTrack?: () => void;
    onNextTrack?: () => void;
  }

  const {
    snap,
    hasMedia,
    audioFileLabel,
    subtitleFileLabel,
    cueTiming,
    overlayVisible,
    playbackRate,
    playlistMode,
    volume,
    onTogglePlayback,
    onSeek,
    onRateChange,
    onPlaylistModeChange,
    onToggleMute,
    onToggleOverlayVisible,
    onVolumeChange,
    onVolumeCommit,
    onPrevTrack,
    onNextTrack,
  }: Props = $props();

  const dur = $derived(Math.max(snap.durationMs, 0));
  const progress = $derived(Math.min(snap.currentTimeMs, dur || snap.currentTimeMs));
  const isMuted = $derived(volume <= 0.001);
  const seekProgressPercent = $derived(
    dur > 0 ? `${Math.max(0, Math.min(100, (progress / dur) * 100))}%` : "0%",
  );
  const volumeProgressPercent = $derived(
    `${Math.max(0, Math.min(100, volume * 100))}%`,
  );

  function thumbGradient(index: number): string {
    const hue = (index * 47) % 360;
    const deg = 135 + (index * 15) % 90;
    return `linear-gradient(${deg}deg, hsla(${hue}, 40%, 45%, 0.5), hsla(${(hue + 30) % 360}, 30%, 12%, 0.9))`;
  }
</script>

<section class="player-dock" data-has-media={hasMedia}>
  <div class="player-bar-inner">
    <div class="bar-left">
      <span class="bar-mini-thumb" style="background: {thumbGradient(0)}"></span>
      <div class="bar-mini-info">
        <span class="bar-mini-title">{audioFileLabel}</span>
        <span class="bar-mini-sub">{subtitleFileLabel} · {cueTiming}</span>
      </div>
    </div>

    <div class="bar-center">
      <div class="bar-controls">
        <button
          class="ctrl-btn ctrl-btn-sm"
          type="button"
          title={playlistMode === "single" ? "单曲循环" : "顺序播放"}
          onclick={() => onPlaylistModeChange(playlistMode === "single" ? "sequential" : "single")}
        >
          {#if playlistMode === "single"}
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><polyline points="17 1 21 5 17 9"/><path d="M3 11V9a4 4 0 0 1 4-4h14"/><polyline points="7 23 3 19 7 15"/><path d="M21 13v2a4 4 0 0 1-4 4H3"/><text x="12" y="14" text-anchor="middle" font-size="8" fill="currentColor" stroke="none" font-weight="700">1</text></svg>
          {:else}
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><polyline points="17 1 21 5 17 9"/><path d="M3 11V9a4 4 0 0 1 4-4h14"/><polyline points="7 23 3 19 7 15"/><path d="M21 13v2a4 4 0 0 1-4 4H3"/></svg>
          {/if}
        </button>

        <button
          class="ctrl-btn"
          type="button"
          disabled={!hasMedia}
          title="上一曲"
          onclick={() => onPrevTrack?.()}
        >
          <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor"><path d="M6 6h2v12H6zm3.5 6l8.5 6V6z"/></svg>
        </button>

        <button
          class="ctrl-btn ctrl-btn-play"
          type="button"
          disabled={!hasMedia}
          title="播放/暂停 (空格)"
          onclick={onTogglePlayback}
        >
          {#if snap.playing}
            <svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor"><rect x="6" y="4" width="4" height="16" rx="1"/><rect x="14" y="4" width="4" height="16" rx="1"/></svg>
          {:else}
            <svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor"><polygon points="5 3 19 12 5 21 5 3"/></svg>
          {/if}
        </button>

        <button
          class="ctrl-btn"
          type="button"
          disabled={!hasMedia}
          title="下一曲"
          onclick={() => onNextTrack?.()}
        >
          <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor"><path d="M6 18l8.5-6L6 6v12zM16 6v12h2V6h-2z"/></svg>
        </button>

        <button
          class="rate-badge"
          type="button"
          onclick={() => {
            const rates = [0.5, 0.75, 0.8, 1, 1.25, 1.5, 2];
            const idx = rates.indexOf(playbackRate);
            const next = rates[(idx + 1) % rates.length];
            onRateChange(next);
          }}
        >
          {playbackRate.toFixed(2)}x
        </button>
      </div>

      <div class="bar-seek">
        <span class="bar-time">{formatDuration(snap.currentTimeMs)}</span>
        <div class="bar-seek-wrap">
          <input
            type="range"
            min="0"
            max={Math.max(dur, 1)}
            value={progress}
            style={`--range-progress: ${seekProgressPercent};`}
            disabled={!hasMedia}
            oninput={(e) => onSeek(Number((e.target as HTMLInputElement).value))}
          />
        </div>
        <span class="bar-time">{formatDuration(dur)}</span>
      </div>
    </div>

    <div class="bar-right">
      <button
        class="vol-toggle-btn"
        class:vol-toggle-btn-muted={overlayVisible}
        type="button"
        title={overlayVisible ? "关闭悬浮窗" : "显示悬浮窗"}
        aria-label={overlayVisible ? "关闭悬浮窗" : "显示悬浮窗"}
        aria-pressed={overlayVisible}
        onclick={onToggleOverlayVisible}
      >
        <svg class="vol-icon" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <rect x="3" y="5" width="18" height="14" rx="2"/>
          <path d="M7 9h10"/>
          <path d="M7 13h6"/>
        </svg>
      </button>
      <button
        class="vol-toggle-btn"
        class:vol-toggle-btn-muted={isMuted}
        type="button"
        title={isMuted ? "恢复音量" : "静音"}
        aria-label={isMuted ? "恢复音量" : "静音"}
        aria-pressed={isMuted}
        onclick={onToggleMute}
      >
        {#if isMuted}
          <svg class="vol-icon" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <polygon points="11 5 6 9 2 9 2 15 6 15 11 19 11 5"/>
            <line x1="23" y1="9" x2="17" y2="15"/>
            <line x1="17" y1="9" x2="23" y2="15"/>
          </svg>
        {:else}
          <svg class="vol-icon" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <polygon points="11 5 6 9 2 9 2 15 6 15 11 19 11 5"/>
            <path d="M15.54 8.46a5 5 0 0 1 0 7.07"/>
          </svg>
        {/if}
      </button>
      <input
        class="vol-slider"
        type="range"
        min="0"
        max="1"
        step="0.01"
        value={volume}
        style={`--range-progress: ${volumeProgressPercent};`}
        oninput={(e) => onVolumeChange(Number((e.target as HTMLInputElement).value))}
        onchange={onVolumeCommit}
      />
      <strong class="vol-pct">{Math.round(volume * 100)}%</strong>
    </div>
  </div>
</section>

<style>
  .player-dock {
    grid-area: player;
    display: flex;
    align-items: stretch;
    width: 100%;
    border-top: 1px solid var(--border);
    background:
      linear-gradient(180deg, rgba(255, 255, 255, 0.03), transparent 55%),
      var(--bg-glass);
    backdrop-filter: blur(16px) saturate(1.6);
    -webkit-backdrop-filter: blur(16px) saturate(1.6);
    box-shadow: 0 -12px 32px rgba(0, 0, 0, 0.22);
    z-index: var(--z-sticky);
  }

  .player-bar-inner {
    width: 100%;
    display: grid;
    grid-template-columns: minmax(220px, 280px) minmax(0, 1fr) minmax(180px, 220px);
    gap: 20px;
    padding: 12px 24px 14px;
    align-items: center;
  }

  .bar-left,
  .bar-center,
  .bar-right {
    min-width: 0;
  }

  .bar-left {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .bar-mini-thumb {
    width: 44px;
    height: 44px;
    border-radius: 8px;
    flex-shrink: 0;
  }

  .bar-mini-info {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .bar-mini-title {
    font-size: var(--font-2xs);
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    color: var(--text-primary);
  }

  .bar-mini-sub {
    font-size: 10px;
    color: var(--text-dim);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .bar-center {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
  }

  .bar-controls {
    display: flex;
    align-items: center;
    gap: 14px;
  }

  .ctrl-btn {
    width: 30px;
    height: 30px;
    border-radius: 50%;
    border: none;
    background: rgba(255, 255, 255, 0.05);
    color: rgba(232, 230, 224, 0.6);
    cursor: pointer;
    display: grid;
    place-items: center;
    transition: all 150ms;
  }

  .ctrl-btn:hover {
    background: rgba(255, 255, 255, 0.1);
    color: var(--text-primary);
  }

  .ctrl-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .ctrl-btn-play {
    width: 40px;
    height: 40px;
    background: var(--accent);
    color: var(--bg-base);
    box-shadow: 0 2px 12px rgba(var(--accent-rgb), 0.25);
  }

  .ctrl-btn-play:hover {
    background: var(--accent-hover);
    box-shadow: 0 4px 20px rgba(var(--accent-rgb), 0.35);
  }

  .ctrl-btn-sm {
    width: 28px;
    height: 28px;
  }

  .rate-badge {
    padding: 3px 8px;
    border-radius: 10px;
    background: rgba(255, 255, 255, 0.05);
    font-size: 11px;
    font-weight: 500;
    color: var(--text-dim);
    cursor: pointer;
    border: none;
    font-family: inherit;
    transition: all 150ms;
  }

  .rate-badge:hover {
    background: rgba(255, 255, 255, 0.1);
    color: var(--text-secondary);
  }

  .bar-seek {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    max-width: 480px;
  }

  .bar-time {
    font-size: 10px;
    color: var(--text-ghost, rgba(232, 230, 224, 0.22));
    font-variant-numeric: tabular-nums;
    min-width: 36px;
  }

  .bar-time:last-child {
    text-align: right;
  }

  .bar-seek-wrap {
    flex: 1;
  }

  .bar-seek-wrap input[type="range"] {
    width: 100%;
    height: 4px;
    background: linear-gradient(
      90deg,
      var(--accent) 0%,
      var(--accent) var(--range-progress, 0%),
      var(--bg-surface-hover) var(--range-progress, 0%),
      var(--bg-surface-hover) 100%
    );
    border-radius: 2px;
    -webkit-appearance: none;
    appearance: none;
    cursor: pointer;
  }

  .bar-seek-wrap input[type="range"]::-webkit-slider-thumb {
    -webkit-appearance: none;
    width: 12px;
    height: 12px;
    border-radius: 50%;
    background: var(--accent);
    border: 2px solid var(--bg-base);
    box-shadow: 0 1px 4px rgba(0, 0, 0, 0.3);
    transition: transform 120ms ease;
  }

  .bar-seek-wrap input[type="range"]:hover::-webkit-slider-thumb {
    transform: scale(1.3);
    box-shadow: 0 0 0 4px var(--accent-soft), 0 2px 8px rgba(0, 0, 0, 0.3);
  }

  .bar-right {
    display: flex;
    align-items: center;
    gap: 8px;
    justify-content: flex-end;
  }

  .vol-toggle-btn {
    width: 28px;
    height: 28px;
    padding: 0;
    display: grid;
    place-items: center;
    border: none;
    border-radius: 50%;
    background: transparent;
    color: var(--text-dim);
    cursor: pointer;
    transition: background 150ms, color 150ms;
    flex-shrink: 0;
  }

  .vol-toggle-btn:hover {
    background: rgba(255, 255, 255, 0.08);
    color: var(--text-primary);
  }

  .vol-toggle-btn-muted {
    color: var(--accent);
    background: var(--accent-soft);
  }

  .vol-toggle-btn-muted:hover {
    color: var(--accent-hover);
    background: rgba(var(--accent-rgb), 0.14);
  }

  .vol-icon {
    flex-shrink: 0;
  }

  .vol-slider {
    width: 70px;
    height: 4px;
    -webkit-appearance: none;
    appearance: none;
    background: linear-gradient(
      90deg,
      var(--accent) 0%,
      var(--accent) var(--range-progress, 0%),
      var(--bg-surface-hover) var(--range-progress, 0%),
      var(--bg-surface-hover) 100%
    );
    border-radius: 2px;
    cursor: pointer;
  }

  .vol-slider::-webkit-slider-thumb {
    -webkit-appearance: none;
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: var(--accent);
    border: 2px solid var(--bg-base);
    transition: transform 120ms ease;
  }

  .vol-slider:hover::-webkit-slider-thumb {
    transform: scale(1.3);
  }

  .vol-pct {
    font-size: 11px;
    font-weight: 500;
    color: var(--text-dim);
    min-width: 32px;
    text-align: right;
  }

  @media (max-width: 900px) {
    .player-bar-inner {
      grid-template-columns: 1fr;
      gap: 10px;
    }

    .bar-left {
      display: none;
    }

    .bar-right {
      justify-content: center;
    }
  }

  @media (max-width: 600px) {
    .player-bar-inner {
      padding: 10px 16px;
    }
  }
</style>
