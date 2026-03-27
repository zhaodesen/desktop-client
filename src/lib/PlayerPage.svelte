<script lang="ts">
  import { tick } from "svelte";
  import { fly } from "svelte/transition";
  import { flip } from "svelte/animate";
  import type { PlaybackSnapshot, PlaybackHistoryItem, PlaylistMode, SubtitleCue } from "../shared/types";
  import { formatDuration } from "../shared/utils";

  type TabId = "lyrics" | "playlist";

  interface Props {
    snap: PlaybackSnapshot;
    hasMedia: boolean;
    audioFileLabel: string;
    subtitleFileLabel: string;
    cueTiming: string;
    currentText: string;
    currentSecondaryText: string;
    subtitleCues: SubtitleCue[];
    playbackRate: number;
    playlistMode: PlaylistMode;
    playlist: PlaybackHistoryItem[];
    pendingPlaylistMediaId: string | undefined;
    currentMediaId: string | undefined;
    volume: number;
    onTogglePlayback: () => void;
    onToggleCurrentItem: () => void;
    onSeek: (ms: number) => void;
    onRateChange: (rate: number) => void;
    onPlaylistModeChange: (mode: PlaylistMode) => void;
    onPlayItem: (id: string) => void;
    onRemoveItem: (id: string) => void;
    onVolumeChange: (volume: number) => void;
    onVolumeCommit: () => void;
  }

  const {
    snap, hasMedia, audioFileLabel, subtitleFileLabel, cueTiming,
    currentText, currentSecondaryText, subtitleCues, playbackRate, playlistMode,
    playlist, pendingPlaylistMediaId, currentMediaId, volume,
    onTogglePlayback, onToggleCurrentItem, onSeek, onRateChange, onPlaylistModeChange,
    onPlayItem, onRemoveItem, onVolumeChange, onVolumeCommit,
  }: Props = $props();

  let activeTab = $state<TabId>("lyrics");
  let lyricScrollEl = $state<HTMLElement | null>(null);
  let lastScrolledCueId = -1;

  const dur = $derived(Math.max(snap.durationMs, 0));
  const progress = $derived(Math.min(snap.currentTimeMs, dur || snap.currentTimeMs));
  const progressPercent = $derived(dur > 0 ? (progress / dur) * 100 : 0);

  /* Find the active cue index based on current playback time */
  const activeCueIndex = $derived.by(() => {
    const t = snap.currentTimeMs;
    for (let i = 0; i < subtitleCues.length; i++) {
      if (t >= subtitleCues[i].startMs && t <= subtitleCues[i].endMs) return i;
    }
    return -1;
  });

  /* Auto-scroll to active lyric line */
  $effect(() => {
    if (activeCueIndex < 0 || !lyricScrollEl || activeTab !== "lyrics") return;
    const cue = subtitleCues[activeCueIndex];
    if (!cue || cue.id === lastScrolledCueId) return;
    lastScrolledCueId = cue.id;

    void tick().then(() => {
      const el = lyricScrollEl?.querySelector(`[data-cue-id="${cue.id}"]`);
      el?.scrollIntoView({ behavior: "smooth", block: "center" });
    });
  });

  function formatMs(ms: number): string {
    const s = Math.floor(ms / 1000);
    const m = Math.floor(s / 60);
    const sec = s % 60;
    return `${String(m).padStart(2, "0")}:${String(sec).padStart(2, "0")}`;
  }
</script>

<section class="page playlist-page" data-active="true">

  <!-- ── Now Playing Header (compact) ── -->
  <div class="now-header">
    <button
      class="now-play-icon"
      type="button"
      disabled={!hasMedia}
      title="播放/暂停"
      onclick={onTogglePlayback}
    >
      {#if snap.playing}
        <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor"><rect x="6" y="4" width="4" height="16" rx="1"/><rect x="14" y="4" width="4" height="16" rx="1"/></svg>
      {:else}
        <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor"><polygon points="8 5 19 12 8 19 8 5"/></svg>
      {/if}
    </button>
    <div class="now-info">
      <div class="now-title">{audioFileLabel}</div>
      <div class="now-tags">
        <span class="now-tag now-tag-accent">{subtitleFileLabel}</span>
        <span class="now-tag">{cueTiming}</span>
      </div>
    </div>
  </div>

  <!-- ── Pill Toggle ── -->
  <div class="pill-toggle">
    <div
      class="pill-toggle-track"
      style="transform: translateX({activeTab === 'lyrics' ? '0' : 'calc(100% + 2px)'})"
    ></div>
    <button
      class="pill-btn"
      class:pill-btn-active={activeTab === "lyrics"}
      type="button"
      onclick={() => { activeTab = "lyrics"; }}
    >
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/></svg>
      字幕
    </button>
    <button
      class="pill-btn"
      class:pill-btn-active={activeTab === "playlist"}
      type="button"
      onclick={() => { activeTab = "playlist"; }}
    >
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="8" y1="6" x2="21" y2="6"/><line x1="8" y1="12" x2="21" y2="12"/><line x1="8" y1="18" x2="21" y2="18"/><line x1="3" y1="6" x2="3.01" y2="6"/><line x1="3" y1="12" x2="3.01" y2="12"/><line x1="3" y1="18" x2="3.01" y2="18"/></svg>
      播放列表
      <span class="pill-badge">{playlist.length}</span>
    </button>
  </div>

  <!-- ── Tab Content ── -->
  {#key activeTab}
  <div class="tab-body" in:fly={{ y: 6, duration: 180 }}>

    {#if activeTab === "lyrics"}
      <!-- LYRIC VIEW -->
      {#if subtitleCues.length === 0}
        <div class="tab-empty">
          <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" opacity="0.35"><path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/></svg>
          <span>暂无字幕，导入资源后自动生成</span>
        </div>
      {:else}
        <div class="lyric-scroll" bind:this={lyricScrollEl}>
          <div class="lyric-list">
            {#each subtitleCues as cue, i (cue.id)}
              <button
                class="lyric-line"
                class:lyric-past={i < activeCueIndex}
                class:lyric-active={i === activeCueIndex}
                type="button"
                data-cue-id={cue.id}
                onclick={() => onSeek(cue.startMs)}
              >
                <span class="lyric-time">{formatMs(cue.startMs)}</span>
                <span class="lyric-original">{cue.text}</span>
                {#if cue.secondaryText}
                  <span class="lyric-translation">{cue.secondaryText}</span>
                {/if}
              </button>
            {/each}
          </div>
        </div>
      {/if}

    {:else}
      <!-- PLAYLIST VIEW -->
      {#if playlist.length === 0}
        <div class="tab-empty">
          <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" opacity="0.35"><circle cx="12" cy="12" r="10"/><polyline points="12 6 12 12 16 14"/></svg>
          <span>播放列表为空，从资源列表添加</span>
        </div>
      {:else}
        <div class="playlist-scroll">
          <div class="playlist-list">
            {#each playlist as entry (entry.mediaId)}
              <div class="playlist-row" in:fly={{ y: 12, duration: 200 }} out:fly={{ x: -30, duration: 150 }} animate:flip={{ duration: 250 }}>
                <button
                  class="playlist-item"
                  class:playlist-item-active={entry.mediaId === currentMediaId}
                  disabled={pendingPlaylistMediaId === entry.mediaId}
                  onclick={() => {
                    if (pendingPlaylistMediaId === entry.mediaId) return;
                    if (entry.mediaId === currentMediaId) onToggleCurrentItem();
                    else onPlayItem(entry.mediaId);
                  }}
                >
                  <div class="playlist-item-indicator">
                    {#if entry.mediaId === currentMediaId && snap.playing}
                      <div class="waveform">
                        <div class="waveform-bar"></div>
                        <div class="waveform-bar"></div>
                        <div class="waveform-bar"></div>
                        <div class="waveform-bar"></div>
                      </div>
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
        </div>
      {/if}
    {/if}

  </div>
  {/key}

  <!-- ── Player Bar ── -->
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
    gap: 0;
    padding-bottom: 150px;
    overflow: hidden;
  }

  /* ── Now Playing Header ── */
  .now-header {
    display: flex;
    align-items: center;
    gap: 12px;
    padding-bottom: 14px;
    flex-shrink: 0;
  }

  .now-play-icon {
    width: 38px;
    height: 38px;
    border-radius: 50%;
    background: var(--accent);
    display: grid;
    place-items: center;
    flex-shrink: 0;
    border: none;
    color: white;
    cursor: pointer;
    box-shadow: 0 2px 12px rgba(var(--accent-rgb), 0.25);
    transition: box-shadow var(--transition-fast), transform 80ms;
  }
  .now-play-icon:hover { box-shadow: 0 4px 20px rgba(var(--accent-rgb), 0.35); }
  .now-play-icon:active { transform: scale(0.95); }
  .now-play-icon:disabled { opacity: 0.5; cursor: not-allowed; }

  .now-info { flex: 1; min-width: 0; }
  .now-title {
    font-size: var(--font-sm);
    font-weight: 600;
    color: var(--text-primary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    margin-bottom: 3px;
  }
  .now-tags { display: flex; gap: 6px; }
  .now-tag {
    font-size: var(--font-2xs);
    padding: 2px 7px;
    border-radius: 6px;
    background: var(--bg-surface);
    color: var(--text-dim);
  }
  .now-tag-accent {
    background: var(--accent-soft);
    color: var(--accent);
  }

  /* ── Pill Toggle ── */
  .pill-toggle {
    display: flex;
    background: var(--bg-surface);
    border-radius: 10px;
    padding: 3px;
    gap: 2px;
    position: relative;
    margin-bottom: 14px;
    flex-shrink: 0;
  }

  .pill-toggle-track {
    position: absolute;
    top: 3px;
    left: 3px;
    width: calc(50% - 2.5px);
    height: calc(100% - 6px);
    border-radius: 8px;
    background: var(--bg-raised);
    border: 1px solid rgba(255, 255, 255, 0.06);
    box-shadow: 0 1px 4px rgba(0, 0, 0, 0.25);
    transition: transform 280ms cubic-bezier(0.4, 0, 0.2, 1);
    z-index: 0;
    pointer-events: none;
  }

  .pill-btn {
    flex: 1;
    padding: 7px 0;
    font-size: var(--font-2xs);
    font-weight: 500;
    border: none;
    background: none;
    color: var(--text-dim);
    cursor: pointer;
    border-radius: 8px;
    position: relative;
    z-index: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    transition: color 200ms;
    user-select: none;
    font-family: inherit;
  }
  .pill-btn:hover { color: var(--text-secondary); }
  .pill-btn-active { color: var(--text-primary); font-weight: 600; }

  .pill-badge {
    font-size: 10px;
    font-weight: 600;
    padding: 0 6px;
    border-radius: 8px;
    background: rgba(var(--accent-rgb), 0.12);
    color: var(--accent);
    line-height: 16px;
  }

  /* ── Tab body ── */
  .tab-body {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .tab-empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    flex: 1;
    gap: 12px;
    color: var(--text-dim);
    font-size: var(--font-sm);
    padding: 40px 20px;
  }

  /* ── Lyric View ── */
  .lyric-scroll {
    flex: 1;
    overflow-y: auto;
    mask-image: linear-gradient(to bottom, transparent 0%, black 8%, black 92%, transparent 100%);
    -webkit-mask-image: linear-gradient(to bottom, transparent 0%, black 8%, black 92%, transparent 100%);
  }

  .lyric-list {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 30px 0;
  }

  .lyric-line {
    display: flex;
    flex-direction: column;
    gap: 0;
    padding: 8px 14px;
    border-radius: 10px;
    border: none;
    background: transparent;
    color: inherit;
    font: inherit;
    text-align: left;
    width: 100%;
    cursor: pointer;
    transition: background 200ms, transform 200ms;
  }
  .lyric-line:hover { background: rgba(255, 255, 255, 0.03); }

  .lyric-time {
    font-size: 10px;
    color: var(--text-ghost, rgba(232, 230, 224, 0.22));
    font-variant-numeric: tabular-nums;
    margin-bottom: 2px;
    transition: color 400ms;
  }

  .lyric-original {
    font-size: var(--font-sm);
    font-weight: 500;
    line-height: 1.6;
    color: var(--text-ghost, rgba(232, 230, 224, 0.22));
    transition: color 400ms, font-size 300ms;
  }

  .lyric-translation {
    font-size: var(--font-2xs);
    line-height: 1.5;
    color: transparent;
    max-height: 0;
    overflow: hidden;
    transition: color 400ms, max-height 300ms, margin-top 300ms;
    margin-top: 0;
  }

  /* Past lines */
  .lyric-past .lyric-original { color: var(--text-dim); }
  .lyric-past .lyric-time { color: var(--text-dim); opacity: 0.4; }

  /* Active line */
  .lyric-active {
    background: rgba(var(--accent-rgb), 0.06);
    transform: scale(1.01);
  }
  .lyric-active .lyric-time { color: var(--accent); opacity: 0.5; }
  .lyric-active .lyric-original {
    font-size: var(--font-base);
    font-weight: 600;
    color: var(--text-primary);
  }
  .lyric-active .lyric-translation {
    color: var(--accent);
    opacity: 0.75;
    max-height: 40px;
    margin-top: 4px;
  }

  /* ── Playlist View ── */
  .playlist-scroll { flex: 1; overflow-y: auto; }

  .playlist-list { gap: 2px; }

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
    border-radius: 10px;
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
  .playlist-item:hover { background: var(--bg-surface); }

  .playlist-item-active { background: var(--accent-soft); color: var(--accent); }
  .playlist-item-active:hover { background: var(--accent-soft); }

  .playlist-item-indicator {
    width: 20px;
    display: flex;
    justify-content: center;
    flex-shrink: 0;
    color: var(--accent);
  }
  .playlist-item-num { color: var(--text-dim); }

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
  .playlist-item-meta { font-size: var(--font-2xs); color: var(--text-dim); }

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
  .playlist-row:hover .playlist-remove-btn { opacity: 1; }
  .playlist-remove-btn:hover {
    border-color: var(--danger-border);
    color: var(--danger);
    background: var(--danger-soft);
  }

  /* ── Waveform ── */
  .waveform { display: flex; align-items: center; gap: 2px; height: 18px; }
  .waveform-bar {
    width: 3px;
    border-radius: 1.5px;
    background: var(--accent);
    animation: waveform-pulse 1.2s ease-in-out infinite;
  }
  .waveform-bar:nth-child(1) { height: 7px; animation-delay: 0s; }
  .waveform-bar:nth-child(2) { height: 13px; animation-delay: 0.15s; }
  .waveform-bar:nth-child(3) { height: 18px; animation-delay: 0.3s; }
  .waveform-bar:nth-child(4) { height: 10px; animation-delay: 0.45s; }

  @keyframes waveform-pulse {
    0%, 100% { transform: scaleY(0.5); }
    50% { transform: scaleY(1); }
  }

  /* ── Player Bar ── */
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
  .player-play-btn {
    box-shadow: 0 2px 12px rgba(var(--accent-rgb), 0.2);
    transition: background var(--transition-fast), box-shadow var(--transition-fast), transform 80ms;
    position: relative;
  }
  .player-play-btn:hover { box-shadow: 0 4px 20px rgba(var(--accent-rgb), 0.35); }

  .player-bar-controls .progress-wrap input[type="range"] {
    height: 5px;
    background: var(--bg-surface-hover);
    border-radius: 2.5px;
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

  .inline-field-volume { min-width: 220px; }
  .volume-control { display: flex; align-items: center; gap: 10px; }
  .volume-control input[type="range"] { flex: 1; min-width: 120px; }

  @media (max-width: 900px) {
    .player-bar { left: 0; }
  }
  @media (max-width: 600px) {
    .player-bar-inner { padding: 10px 16px; }
    .player-bar-options { gap: 8px; }
    .inline-field-volume { min-width: 160px; }
  }
</style>
