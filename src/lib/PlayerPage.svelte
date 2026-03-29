<script lang="ts">
  import { fly } from "svelte/transition";
  import { flip } from "svelte/animate";
  import { Virtualizer } from "virtua/svelte";
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
    overlayVisible: boolean;
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
    onToggleMute: () => void;
    onToggleOverlayVisible: () => void;
    onPlayItem: (id: string) => void;
    onRemoveItem: (id: string) => void;
    onVolumeChange: (volume: number) => void;
    onVolumeCommit: () => void;
    onPrevTrack?: () => void;
    onNextTrack?: () => void;
  }

  const {
    snap, hasMedia, audioFileLabel, subtitleFileLabel, cueTiming,
    currentText: _, currentSecondaryText: __,
    subtitleCues, overlayVisible, playbackRate, playlistMode,
    playlist, pendingPlaylistMediaId, currentMediaId, volume,
    onTogglePlayback, onToggleCurrentItem, onSeek, onRateChange, onPlaylistModeChange, onToggleMute, onToggleOverlayVisible,
    onPlayItem, onRemoveItem, onVolumeChange, onVolumeCommit,
    onPrevTrack, onNextTrack,
  }: Props = $props();

  let activeTab = $state<TabId>("lyrics");
  let showBilingual = $state(false);
  let lastScrolledCueId = -1;
  let virtualList = $state<any>(undefined);

  const dur = $derived(Math.max(snap.durationMs, 0));
  const progress = $derived(Math.min(snap.currentTimeMs, dur || snap.currentTimeMs));
  const isMuted = $derived(volume <= 0.001);
  const seekProgressPercent = $derived(
    dur > 0 ? `${Math.max(0, Math.min(100, (progress / dur) * 100))}%` : "0%",
  );
  const volumeProgressPercent = $derived(
    `${Math.max(0, Math.min(100, volume * 100))}%`,
  );

  const activeCueIndex = $derived.by(() => {
    const t = snap.currentTimeMs;
    const cues = subtitleCues;
    let lo = 0;
    let hi = cues.length - 1;
    while (lo <= hi) {
      const mid = (lo + hi) >>> 1;
      if (t >= cues[mid].endMs) lo = mid + 1;
      else if (t < cues[mid].startMs) hi = mid - 1;
      else return mid;
    }
    return -1;
  });

  $effect(() => {
    if (activeCueIndex < 0 || !virtualList || activeTab !== "lyrics") return;
    const cue = subtitleCues[activeCueIndex];
    if (!cue || cue.id === lastScrolledCueId) return;
    lastScrolledCueId = cue.id;
    virtualList.scrollToIndex(activeCueIndex, { align: "center", smooth: true });
  });

  function thumbGradient(index: number): string {
    const hue = (index * 47) % 360;
    const deg = 135 + (index * 15) % 90;
    return `linear-gradient(${deg}deg, hsla(${hue}, 40%, 45%, 0.5), hsla(${(hue + 30) % 360}, 30%, 12%, 0.9))`;
  }
</script>

<section class="page playlist-page" data-active="true">

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
      列表
      <span class="pill-badge">{playlist.length}</span>
    </button>
  </div>

  <!-- ── Tab Content ── -->
  {#key activeTab}
  <div class="tab-body" in:fly={{ y: 6, duration: 180 }}>

    {#if activeTab === "lyrics"}
      <!-- LYRIC VIEW -->
      <div class="lyrics-wrapper" class:bilingual={showBilingual}>
        {#if subtitleCues.some(c => c.secondaryText)}
          <button
            class="lyrics-toggle-btn"
            class:bilingual-active={showBilingual}
            type="button"
            title={showBilingual ? "切换为原文字幕" : "切换为双语字幕"}
            onclick={() => { showBilingual = !showBilingual; }}
          >
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><path d="M5 8l6 6"/><path d="M4 14l6-6 2-3"/><path d="M2 5h12"/><path d="M7 2h1"/><path d="M22 22l-5-10-5 10"/><path d="M14 18h6"/></svg>
          </button>
        {/if}

        {#if subtitleCues.length === 0}
          <div class="tab-empty">
            <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" opacity="0.35"><path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/></svg>
            <span>暂无字幕，导入资源后自动生成</span>
          </div>
        {:else}
          <div class="lyric-scroll">
            <Virtualizer data={subtitleCues} overscan={8} bind:this={virtualList}>
              {#snippet children(cue, i)}
                <button
                  class="lyric-line"
                  class:lyric-past={i < activeCueIndex}
                  class:lyric-active={i === activeCueIndex}
                  type="button"
                  onclick={() => onSeek(cue.startMs)}
                >
                  <span class="lyric-text">{cue.text}</span>
                  {#if showBilingual && cue.secondaryText}
                    <span class="lyric-translation">{cue.secondaryText}</span>
                  {/if}
                </button>
              {/snippet}
            </Virtualizer>
          </div>
        {/if}
      </div>

    {:else}
      <!-- PLAYLIST VIEW -->
      {#if playlist.length === 0}
        <div class="tab-empty">
          <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" opacity="0.35"><circle cx="12" cy="12" r="10"/><polyline points="12 6 12 12 16 14"/></svg>
          <span>播放列表为空，从资源列表添加</span>
        </div>
      {:else}
        <!-- Action Bar -->
        <div class="action-bar">
          <button
            class="btn-play-all"
            type="button"
            disabled={!playlist.length}
            onclick={() => { if (playlist.length) onPlayItem(playlist[0].mediaId); }}
          >
            <svg width="12" height="12" viewBox="0 0 24 24" fill="currentColor"><polygon points="5 3 19 12 5 21 5 3"/></svg>
            全部播放
          </button>
          <div class="action-spacer"></div>
          <span class="track-count">{playlist.length} 首曲目</span>
        </div>

        <!-- Track List Header -->
        <div class="track-header">
          <span class="col-num">#</span>
          <span class="col-thumb"></span>
          <span class="col-title">标题</span>
          <span class="col-sub">字幕</span>
          <span class="col-plays">播放</span>
          <span class="col-dur">时长</span>
        </div>

        <!-- Track List -->
        <div class="playlist-scroll">
          {#each playlist as entry, i (entry.mediaId)}
            <div
              class="track-row"
              class:track-row-active={entry.mediaId === currentMediaId}
              in:fly={{ y: 12, duration: 200 }}
              out:fly={{ x: -30, duration: 150 }}
              animate:flip={{ duration: 250 }}
            >
              <button
                class="track-row-btn"
                disabled={pendingPlaylistMediaId === entry.mediaId}
                onclick={() => {
                  if (pendingPlaylistMediaId === entry.mediaId) return;
                  if (entry.mediaId === currentMediaId) onToggleCurrentItem();
                  else onPlayItem(entry.mediaId);
                }}
              >
                <!-- Number / Waveform -->
                <span class="col-num">
                  {#if entry.mediaId === currentMediaId && snap.playing}
                    <span class="waveform">
                      <span class="waveform-bar"></span>
                      <span class="waveform-bar"></span>
                      <span class="waveform-bar"></span>
                      <span class="waveform-bar"></span>
                    </span>
                  {:else if entry.mediaId === currentMediaId}
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor"><polygon points="5 3 19 12 5 21 5 3"/></svg>
                  {:else}
                    <span class="track-num-text">{i + 1}</span>
                  {/if}
                </span>

                <!-- Thumbnail -->
                <span class="col-thumb">
                  <span class="track-thumb" style="background: {thumbGradient(i)}"></span>
                </span>

                <!-- Title -->
                <span class="col-title">
                  <span class="track-title">{entry.title}</span>
                  <span class="track-meta">{entry.subtitlePath ? "字幕已加载" : "无字幕文件"}</span>
                </span>

                <!-- Subtitle Badge -->
                <span class="col-sub">
                  <span class="sub-badge" class:sub-badge-yes={!!entry.subtitlePath} class:sub-badge-no={!entry.subtitlePath}>
                    {entry.subtitlePath ? "有" : "无"}
                  </span>
                </span>

                <!-- Play Count -->
                <span class="col-plays">{entry.playCount}</span>

                <!-- Duration -->
                <span class="col-dur">—</span>
              </button>

              <button
                class="track-remove-btn"
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
    {/if}

  </div>
  {/key}

  <!-- ── Player Bar ── -->
  <div class="player-bar">
    <div class="player-bar-inner">
      <!-- Left: mini info -->
      <div class="bar-left">
        <span class="bar-mini-thumb" style="background: {thumbGradient(0)}"></span>
        <div class="bar-mini-info">
          <span class="bar-mini-title">{audioFileLabel}</span>
          <span class="bar-mini-sub">{subtitleFileLabel} · {cueTiming}</span>
        </div>
      </div>

      <!-- Center: controls + seek -->
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

      <!-- Right: volume -->
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
  </div>
</section>

<style>
  .playlist-page {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-height: 0;
    height: 100%;
    gap: 0;
    padding-bottom: 80px;
    overflow: hidden;
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
    color: var(--text-secondary);
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
  .pill-btn:hover { color: var(--text-primary); }
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

  /* ── Lyrics View ── */
  .lyrics-wrapper {
    position: relative;
    display: flex;
    flex-direction: column;
    flex: 1;
    min-height: 0;
  }

  .lyrics-toggle-btn {
    position: absolute;
    top: 0;
    right: 0;
    z-index: 2;
    width: 36px;
    height: 36px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border);
    background: var(--bg-surface);
    color: var(--text-dim);
    cursor: pointer;
    display: grid;
    place-items: center;
    transition: all 150ms;
  }
  .lyrics-toggle-btn:hover {
    border-color: var(--border-focus);
    color: var(--text-primary);
    background: var(--bg-surface-hover);
  }
  .lyrics-toggle-btn.bilingual-active {
    color: var(--accent);
    border-color: var(--accent-border);
    background: var(--accent-soft);
  }

  .lyric-scroll {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    scrollbar-gutter: stable;
    overflow-x: hidden;
    padding: 30px 0 84px;
  }

  .lyric-line {
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
    gap: 0;
    padding: 10px 14px;
    border-radius: 10px;
    border: none;
    background: transparent;
    color: inherit;
    font: inherit;
    width: 100%;
    cursor: pointer;
    transition: background 200ms, transform 200ms;
  }
  .lyric-line:hover { background: rgba(255, 255, 255, 0.03); }

  .lyric-text {
    font-size: var(--font-sm);
    font-weight: 500;
    line-height: 1.6;
    text-align: center;
    color: var(--text-ghost, rgba(232, 230, 224, 0.22));
    transition: color 400ms, font-size 300ms;
  }

  .lyric-translation {
    font-size: var(--font-2xs);
    line-height: 1.5;
    text-align: center;
    color: transparent;
    max-height: 0;
    overflow: hidden;
    transition: color 400ms, max-height 300ms, margin-top 300ms;
    margin-top: 0;
  }

  /* Bilingual mode: show all translations */
  .bilingual .lyric-translation {
    color: var(--text-dim);
    opacity: 0.55;
    max-height: 30px;
    margin-top: 3px;
  }
  .bilingual .lyric-active .lyric-translation {
    color: var(--accent);
    opacity: 0.75;
  }

  .lyric-past .lyric-text { color: var(--text-dim); }

  .lyric-active {
    background: rgba(var(--accent-rgb), 0.06);
    transform: scale(1.01);
  }
  .lyric-active .lyric-text {
    font-size: var(--font-base);
    font-weight: 600;
    color: var(--text-primary);
  }
  /* ── Action Bar ── */
  .action-bar {
    display: flex;
    align-items: center;
    gap: 12px;
    flex-shrink: 0;
    margin-bottom: 10px;
  }

  .btn-play-all {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 8px 16px;
    border-radius: 20px;
    border: none;
    background: var(--accent);
    color: var(--bg-base);
    font-size: var(--font-2xs);
    font-weight: 600;
    cursor: pointer;
    font-family: inherit;
    box-shadow: 0 2px 12px rgba(var(--accent-rgb), 0.25);
    transition: background var(--transition-fast), box-shadow var(--transition-fast), transform 80ms;
  }
  .btn-play-all:hover { background: var(--accent-hover); box-shadow: 0 4px 20px rgba(var(--accent-rgb), 0.35); transform: translateY(-1px); }
  .btn-play-all:active { transform: translateY(0) scale(0.97); }
  .btn-play-all:disabled { opacity: 0.5; cursor: not-allowed; }

  .action-spacer { flex: 1; }
  .track-count { font-size: var(--font-2xs); color: var(--text-ghost, rgba(232, 230, 224, 0.22)); }

  /* ── Track Header ── */
  .track-header {
    display: grid;
    grid-template-columns: 28px 48px 1fr 48px 40px 48px;
    gap: 12px;
    padding: 4px 14px;
    align-items: center;
    font-size: 11px;
    font-weight: 500;
    color: var(--text-ghost, rgba(232, 230, 224, 0.22));
    border-bottom: 1px solid rgba(255, 255, 255, 0.05);
    margin-bottom: 4px;
    flex-shrink: 0;
  }
  .track-header .col-sub,
  .track-header .col-plays { text-align: center; }
  .track-header .col-dur { text-align: right; }

  /* ── Playlist Scroll ── */
  .playlist-scroll {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 2px;
    scrollbar-gutter: stable;
  }

  /* ── Track Row ── */
  .track-row {
    position: relative;
    flex-shrink: 0;
  }

  .track-row-btn {
    display: grid;
    grid-template-columns: 28px 48px 1fr 48px 40px 48px;
    gap: 12px;
    padding: 8px 14px;
    align-items: center;
    border: 1px solid transparent;
    border-radius: 10px;
    background: transparent;
    color: var(--text-primary);
    font: inherit;
    cursor: pointer;
    text-align: left;
    width: 100%;
    transition: background 150ms;
  }
  .track-row-btn:hover { background: var(--bg-surface); }

  .track-row-active .track-row-btn {
    background: rgba(var(--accent-rgb), 0.06);
    border-color: rgba(var(--accent-rgb), 0.1);
  }
  .track-row-active .track-row-btn:hover {
    background: rgba(var(--accent-rgb), 0.08);
  }

  .col-num {
    display: flex;
    justify-content: center;
    align-items: center;
    color: var(--text-ghost, rgba(232, 230, 224, 0.22));
  }
  .track-row-active .col-num { color: var(--accent); }
  .track-num-text { font-size: 13px; }

  /* Thumbnail */
  .col-thumb { display: flex; align-items: center; }
  .track-thumb {
    width: 44px;
    height: 44px;
    border-radius: 8px;
    flex-shrink: 0;
    position: relative;
  }

  /* Title column */
  .col-title {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .track-title {
    font-size: var(--font-base);
    font-weight: 500;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    color: var(--text-primary);
  }
  .track-row-active .track-title { color: var(--accent); font-weight: 600; }

  .track-meta {
    font-size: 10px;
    color: var(--text-ghost, rgba(232, 230, 224, 0.22));
  }

  /* Subtitle badge */
  .col-sub { display: flex; justify-content: center; }
  .sub-badge {
    display: inline-flex;
    padding: 3px 8px;
    border-radius: 6px;
    font-size: 10px;
    font-weight: 500;
  }
  .sub-badge-yes { background: rgba(52, 211, 153, 0.1); color: #34d399; }
  .sub-badge-no { background: rgba(255, 255, 255, 0.04); color: var(--text-ghost, rgba(232, 230, 224, 0.22)); }

  /* Play count & duration */
  .col-plays {
    font-size: var(--font-2xs);
    color: var(--text-ghost, rgba(232, 230, 224, 0.22));
    text-align: center;
  }
  .col-dur {
    font-size: var(--font-2xs);
    color: var(--text-dim);
    text-align: right;
    font-variant-numeric: tabular-nums;
  }

  /* Remove button */
  .track-remove-btn {
    position: absolute;
    right: 8px;
    top: 50%;
    transform: translateY(-50%);
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    border: 1px solid transparent;
    background: transparent;
    color: var(--text-dim);
    border-radius: var(--radius-sm);
    cursor: pointer;
    opacity: 0;
    transition: opacity 150ms, border-color 150ms, color 150ms, background 150ms;
  }
  .track-row:hover .track-remove-btn { opacity: 1; }
  .track-remove-btn:hover {
    border-color: var(--danger-border);
    color: var(--danger);
    background: var(--danger-soft);
  }

  /* ── Waveform ── */
  .waveform { display: flex; align-items: flex-end; gap: 2px; height: 18px; }
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
    z-index: var(--z-sticky);
  }

  .player-bar-inner {
    display: grid;
    grid-template-columns: 200px 1fr 160px;
    gap: 20px;
    padding: 12px 24px 14px;
    align-items: center;
  }

  /* Bar Left */
  .bar-left {
    display: flex;
    align-items: center;
    gap: 12px;
    min-width: 0;
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

  /* Bar Center */
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
  .ctrl-btn:hover { background: rgba(255, 255, 255, 0.1); color: var(--text-primary); }
  .ctrl-btn:disabled { opacity: 0.4; cursor: not-allowed; }

  .ctrl-btn-play {
    width: 40px;
    height: 40px;
    background: var(--accent);
    color: var(--bg-base);
    box-shadow: 0 2px 12px rgba(var(--accent-rgb), 0.25);
  }
  .ctrl-btn-play:hover { background: var(--accent-hover); box-shadow: 0 4px 20px rgba(var(--accent-rgb), 0.35); }

  .ctrl-btn-sm { width: 28px; height: 28px; }

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
  .rate-badge:hover { background: rgba(255, 255, 255, 0.1); color: var(--text-secondary); }

  /* Seek bar */
  .bar-seek {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    max-width: 420px;
  }
  .bar-time {
    font-size: 10px;
    color: var(--text-ghost, rgba(232, 230, 224, 0.22));
    font-variant-numeric: tabular-nums;
    min-width: 36px;
  }
  .bar-time:last-child { text-align: right; }

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

  /* Bar Right */
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
  .vol-icon { flex-shrink: 0; }
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
  .vol-slider:hover::-webkit-slider-thumb { transform: scale(1.3); }
  .vol-pct {
    font-size: 11px;
    font-weight: 500;
    color: var(--text-dim);
    min-width: 32px;
    text-align: right;
  }

  /* ── Responsive ── */
  @media (max-width: 900px) {
    .player-bar { left: 0; }
    .player-bar-inner { grid-template-columns: 1fr; gap: 10px; }
    .bar-left { display: none; }
    .bar-right { justify-content: center; }
  }
  @media (max-width: 600px) {
    .player-bar-inner { padding: 10px 16px; }
  }
</style>
