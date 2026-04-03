<script lang="ts">
  import { fly } from "svelte/transition";
  import { flip } from "svelte/animate";
  import { Virtualizer } from "virtua/svelte";
  import { buildDisplayCue, cueNeedsInterAtomSpacing, findCueIndexAtTime, predictPlaybackTime } from "../main/lyric-timing";
  import type {
    PlaybackClockAnchor,
    PlaybackHistoryItem,
    PlaybackSnapshot,
    SubtitleCue,
    SubtitleDisplayMode,
  } from "../shared/types";

  type TabId = "lyrics" | "playlist";

  interface Props {
    snap: PlaybackSnapshot;
    subtitleCues: SubtitleCue[];
    playbackAnchor: PlaybackClockAnchor;
    subtitleDisplayMode: SubtitleDisplayMode;
    playlist: PlaybackHistoryItem[];
    pendingPlaylistMediaId: string | undefined;
    currentMediaId: string | undefined;
    onToggleCurrentItem: () => void;
    onSeek: (ms: number) => void;
    onSubtitleDisplayModeChange: (mode: SubtitleDisplayMode) => void;
    onPlayItem: (id: string) => void;
    onRemoveItem: (id: string) => void;
  }

  const {
    snap,
    subtitleCues,
    playbackAnchor,
    subtitleDisplayMode,
    playlist,
    pendingPlaylistMediaId,
    currentMediaId,
    onToggleCurrentItem,
    onSeek,
    onSubtitleDisplayModeChange,
    onPlayItem,
    onRemoveItem,
  }: Props = $props();

  let activeTab = $state<TabId>("lyrics");
  let lastScrolledCueId = -1;
  let virtualList = $state<any>(undefined);
  let displayTimeMs = $state(0);
  let playbackRaf = 0;

  const displayedCues = $derived.by(() => subtitleCues.map((cue) => buildDisplayCue(cue, subtitleDisplayMode) ?? {
    ...cue,
    atoms: cue.atoms ?? [],
  }));
  const activeCueIndex = $derived(findCueIndexAtTime(subtitleCues, displayTimeMs));

  $effect(() => {
    if (playbackRaf) {
      cancelAnimationFrame(playbackRaf);
      playbackRaf = 0;
    }

    const tick = () => {
      displayTimeMs = predictPlaybackTime(playbackAnchor);
      if (!playbackAnchor.playing) return;
      playbackRaf = requestAnimationFrame(tick);
    };

    displayTimeMs = predictPlaybackTime(playbackAnchor);
    if (playbackAnchor.playing) {
      playbackRaf = requestAnimationFrame(tick);
    }

    return () => {
      if (playbackRaf) {
        cancelAnimationFrame(playbackRaf);
        playbackRaf = 0;
      }
    };
  });

  $effect(() => {
    if (activeCueIndex < 0 || !virtualList || activeTab !== "lyrics") return;
    const cue = displayedCues[activeCueIndex];
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
      <div class="lyrics-wrapper" class:bilingual={subtitleDisplayMode === "bilingual"}>
        {#if subtitleCues.some(c => c.secondaryText)}
          <button
            class="lyrics-toggle-btn"
            class:bilingual-active={subtitleDisplayMode === "bilingual"}
            type="button"
            title={subtitleDisplayMode === "bilingual" ? "切换为原文字幕" : "切换为双语字幕"}
            onclick={() => {
              onSubtitleDisplayModeChange(
                subtitleDisplayMode === "bilingual" ? "original" : "bilingual",
              );
            }}
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
            <Virtualizer data={displayedCues} overscan={8} bind:this={virtualList}>
              {#snippet children(cue, i)}
                <button
                  class="lyric-line"
                  class:lyric-past={i < activeCueIndex}
                  class:lyric-active={i === activeCueIndex}
                  type="button"
                  onclick={() => onSeek(cue.startMs)}
                >
                  {#if i === activeCueIndex && cue.atoms.length > 0}
                    <span class="lyric-text lyric-text-active">
                      {#each cue.atoms as atom, atomIndex (`${cue.id}-${atom.startMs}-${atom.endMs}-${atomIndex}`)}
                        <span
                          class="lyric-atom"
                          class:lyric-atom-filled={displayTimeMs >= atom.endMs}
                          class:lyric-atom-active={displayTimeMs >= atom.startMs && displayTimeMs < atom.endMs}
                        >
                          {atom.text}{cueNeedsInterAtomSpacing(cue.text) && atomIndex < cue.atoms.length - 1 ? " " : ""}
                        </span>
                      {/each}
                    </span>
                  {:else}
                    <span class="lyric-text" class:lyric-text-active={i === activeCueIndex}>
                      {cue.text}
                    </span>
                  {/if}
                  {#if subtitleDisplayMode === "bilingual" && cue.secondaryText}
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
</section>

<style>
  .playlist-page {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-height: 0;
    height: 100%;
    gap: 0;
    overflow: hidden;
  }

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
    display: block;
    font-size: var(--font-sm);
    font-weight: 500;
    line-height: 1.6;
    text-align: center;
    color: var(--text-ghost, rgba(232, 230, 224, 0.22));
    width: 100%;
    white-space: normal;
    overflow-wrap: anywhere;
    transition: font-size 300ms, font-weight 220ms, color 180ms ease;
  }

  .lyric-text-active {
    color: var(--text-primary);
    text-shadow:
      0 0 10px rgba(var(--accent-rgb), 0.08),
      0 0 24px rgba(var(--accent-rgb), 0.04);
    transition: font-size 300ms, font-weight 220ms, color 180ms ease, text-shadow 180ms ease;
  }

  .lyric-atom {
    color: color-mix(in srgb, var(--text-primary) 24%, transparent);
    opacity: 0.42;
    filter: saturate(0.82) brightness(0.92);
    transition:
      color 220ms cubic-bezier(0.22, 1, 0.36, 1),
      text-shadow 320ms cubic-bezier(0.22, 1, 0.36, 1),
      opacity 220ms ease,
      filter 260ms ease;
  }

  .lyric-atom-filled {
    color: color-mix(in srgb, var(--accent) 24%, white);
    opacity: 0.78;
    filter: saturate(1) brightness(0.98);
    text-shadow:
      0 0 8px rgba(255, 255, 255, 0.08),
      0 0 18px rgba(var(--accent-rgb), 0.1);
  }

  .lyric-atom-active {
    color: white;
    opacity: 1;
    filter: saturate(1.08) brightness(1.06);
    text-shadow:
      0 0 14px rgba(255, 255, 255, 0.22),
      0 0 30px rgba(var(--accent-rgb), 0.16);
  }

  .lyric-translation {
    display: block;
    font-size: var(--font-2xs);
    line-height: 1.5;
    text-align: center;
    color: transparent;
    max-height: 0;
    overflow: hidden;
    white-space: normal;
    overflow-wrap: anywhere;
    transition: color 400ms, max-height 300ms, margin-top 300ms;
    margin-top: 0;
  }

  /* Bilingual mode: show all translations */
  .bilingual .lyric-translation {
    color: var(--text-dim);
    opacity: 0.55;
    max-height: 3.2em;
    margin-top: 3px;
  }
  .bilingual .lyric-active .lyric-translation {
    color: var(--accent);
    opacity: 0.75;
  }

  .lyric-active {
    background: rgba(var(--accent-rgb), 0.06);
    transform: scale(1.025);
  }
  .lyric-active .lyric-text {
    font-size: var(--font-base);
    font-weight: 600;
    color: var(--text-primary);
    text-shadow:
      0 0 14px rgba(var(--accent-rgb), 0.22),
      0 0 28px rgba(var(--accent-rgb), 0.14),
      0 0 44px rgba(255, 255, 255, 0.08);
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
</style>
