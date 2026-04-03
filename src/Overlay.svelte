<script lang="ts">
  import { onMount } from "svelte";
  import { LogicalSize } from "@tauri-apps/api/dpi";
  import { emitTo, listen } from "@tauri-apps/api/event";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import {
    OVERLAY_CLEAR_EVENT,
    OVERLAY_CLOSE_EVENT,
    OVERLAY_LOCK_EVENT,
    OVERLAY_RENDER_EVENT,
    OVERLAY_STYLE_EVENT,
  } from "./shared/events";
  import { buildDisplayCue, cueNeedsInterAtomSpacing, predictPlaybackTime } from "./main/lyric-timing";
  import type { OverlayRenderPayload, OverlaySettings, PlaybackClockAnchor, SubtitleCue, SubtitleDisplayMode } from "./shared/types";
  import { formatDuration } from "./shared/utils";
  import "./overlay.css";

  let locked = $state(false);
  let position = $state("bottom");

  let fileLabel = $state("未选择素材");
  let currentCue = $state<SubtitleCue | undefined>(undefined);
  let subtitleDisplayMode = $state<SubtitleDisplayMode>("bilingual");
  let playbackAnchor = $state<PlaybackClockAnchor>({
    mediaTimeMs: 0,
    wallTimeMs: Date.now(),
    durationMs: 0,
    rate: 1,
    playing: false,
  });
  let displayTimeMs = $state(0);

  let fontSize = $state(34);
  let opacity = $state(1.0);
  let color = $state("#ffffff");
  let strokeColor = $state("#000000");
  let secondaryColor = $state("#ffffff");
  let secondaryStroke = $state("#000000");
  let shellElement: HTMLElement | undefined;

  const tauriWindow = getCurrentWindow();
  const overlayVerticalPadding = 24;
  const minOverlayHeight = 72;
  let resizeObserver: ResizeObserver | undefined;
  let resizeRaf = 0;
  let lastAppliedHeight = 0;
  let playbackRaf = 0;

  const displayedCue = $derived(buildDisplayCue(currentCue, subtitleDisplayMode));
  const currentText = $derived(displayedCue?.text ?? "当前时间点暂无字幕");
  const secondaryText = $derived(displayedCue?.secondaryText ?? "");
  const currentAtoms = $derived(displayedCue?.atoms ?? []);
  const needsInterAtomSpacing = $derived(cueNeedsInterAtomSpacing(currentText));
  const metaText = $derived([
    fileLabel || "未选择素材",
    playbackAnchor.playing ? "播放中" : "已暂停",
    formatDuration(displayTimeMs),
  ].join(" · "));

  function refreshPlaybackTime(now = Date.now()) {
    displayTimeMs = predictPlaybackTime(playbackAnchor, now);
  }

  function schedulePlaybackFrame() {
    if (playbackRaf) {
      cancelAnimationFrame(playbackRaf);
    }

    const tick = () => {
      refreshPlaybackTime();
      if (!playbackAnchor.playing) {
        playbackRaf = 0;
        return;
      }
      playbackRaf = requestAnimationFrame(tick);
    };

    refreshPlaybackTime();
    if (playbackAnchor.playing) {
      playbackRaf = requestAnimationFrame(tick);
    } else {
      playbackRaf = 0;
    }
  }

  async function syncWindowHeight() {
    if (!shellElement) return;

    const desiredHeight = Math.max(
      minOverlayHeight,
      Math.ceil(shellElement.getBoundingClientRect().height + overlayVerticalPadding),
    );

    if (Math.abs(desiredHeight - lastAppliedHeight) < 1) {
      return;
    }

    const scaleFactor = await tauriWindow.scaleFactor();
    const logicalSize = (await tauriWindow.innerSize()).toLogical(scaleFactor);

    if (Math.abs(logicalSize.height - desiredHeight) < 1) {
      lastAppliedHeight = desiredHeight;
      return;
    }

    await tauriWindow.setSize(new LogicalSize(logicalSize.width, desiredHeight));
    lastAppliedHeight = desiredHeight;
  }

  function scheduleWindowHeightSync() {
    if (resizeRaf) {
      cancelAnimationFrame(resizeRaf);
    }

    resizeRaf = requestAnimationFrame(() => {
      resizeRaf = 0;
      void syncWindowHeight();
    });
  }

  function applyStyle(settings: OverlaySettings) {
    fontSize = settings.fontSize;
    opacity = settings.opacity;
    color = settings.color;
    strokeColor = settings.strokeColor;
    secondaryColor = settings.secondaryColor;
    secondaryStroke = settings.secondaryStrokeColor;
    position = settings.position;
  }

  function render(payload: OverlayRenderPayload) {
    currentCue = payload.current;
    subtitleDisplayMode = payload.subtitleDisplayMode;
    playbackAnchor = payload.playbackAnchor;
    fileLabel = payload.fileLabel ?? "未选择素材";
    schedulePlaybackFrame();
  }

  function clear() {
    currentCue = undefined;
    fileLabel = "未选择素材";
    playbackAnchor = {
      mediaTimeMs: 0,
      wallTimeMs: Date.now(),
      durationMs: 0,
      rate: 1,
      playing: false,
    };
    refreshPlaybackTime();
  }

  async function applyLockState(newLocked: boolean) {
    locked = newLocked;
    await tauriWindow.setIgnoreCursorEvents(newLocked);
  }

  function handleMousedown(e: MouseEvent) {
    if (locked) return;
    const target = e.target as HTMLElement;
    if (target.closest(".overlay-controls")) return;
    if (e.button !== 0) return;
    e.preventDefault();
    void tauriWindow.startDragging();
  }

  function handleDblclick(e: MouseEvent) {
    e.preventDefault();
    e.stopPropagation();
  }

  async function handleClose() {
    await emitTo("main", OVERLAY_CLOSE_EVENT, {});
    await tauriWindow.hide();
  }

  async function handleLock() {
    await applyLockState(true);
    void emitTo("main", OVERLAY_LOCK_EVENT, { locked: true });
  }

  onMount(async () => {
    void getCurrentWebview().setBackgroundColor({ red: 0, green: 0, blue: 0, alpha: 0 });

    await applyLockState(false);

    const unlistenRender = await listen<OverlayRenderPayload>(OVERLAY_RENDER_EVENT, ({ payload }) => render(payload));
    const unlistenStyle = await listen<OverlaySettings>(OVERLAY_STYLE_EVENT, ({ payload }) => applyStyle(payload));
    const unlistenClear = await listen(OVERLAY_CLEAR_EVENT, () => clear());
    const unlistenLock = await listen<{ locked: boolean }>(OVERLAY_LOCK_EVENT, ({ payload }) => {
      void applyLockState(payload.locked);
    });

    if (shellElement) {
      resizeObserver = new ResizeObserver(() => scheduleWindowHeightSync());
      resizeObserver.observe(shellElement);
    }

    scheduleWindowHeightSync();
    schedulePlaybackFrame();

    return () => {
      resizeObserver?.disconnect();
      if (resizeRaf) {
        cancelAnimationFrame(resizeRaf);
      }
      if (playbackRaf) {
        cancelAnimationFrame(playbackRaf);
      }
      unlistenRender();
      unlistenStyle();
      unlistenClear();
      unlistenLock();
    };
  });
</script>

<!-- svelte-ignore a11y_no_static_element_interactions a11y_no_noninteractive_element_interactions -->
<section
  bind:this={shellElement}
  class="overlay-shell"
  data-position={position}
  data-locked={locked}
  style="
    --overlay-font-size: {fontSize}px;
    --overlay-color: {color};
    --overlay-stroke-color: {strokeColor};
    --overlay-secondary-color: {secondaryColor};
    --overlay-secondary-stroke: {secondaryStroke};
  "
  onmousedown={handleMousedown}
  ondblclick={handleDblclick}
>
  <div class="overlay-controls">
    <button class="overlay-ctrl-btn" title="锁定窗口" onclick={handleLock}>
      <svg class="icon-unlocked" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
        <rect x="3" y="11" width="18" height="11" rx="2" ry="2" />
        <path d="M7 11V7a5 5 0 0 1 9.9-1" />
      </svg>
      <svg class="icon-locked" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
        <rect x="3" y="11" width="18" height="11" rx="2" ry="2" />
        <path d="M7 11V7a5 5 0 0 1 10 0v4" />
      </svg>
    </button>
    <button class="overlay-ctrl-btn overlay-close-btn" title="隐藏悬浮窗" onclick={handleClose}>
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round">
        <line x1="18" y1="6" x2="6" y2="18" />
        <line x1="6" y1="6" x2="18" y2="18" />
      </svg>
    </button>
  </div>

  <div class="overlay-meta" style="opacity: {Math.max(0, opacity * 0.7)}">{metaText}</div>
  <div
    class="overlay-current"
    style="
      opacity: {opacity};
    "
  >
    {#if currentAtoms.length > 0}
      {#each currentAtoms as atom, index (`${atom.startMs}-${atom.endMs}-${index}`)}
        <span
          class="overlay-current-char"
          class:overlay-current-char-filled={displayTimeMs >= atom.endMs}
          class:overlay-current-char-active={displayTimeMs >= atom.startMs && displayTimeMs < atom.endMs}
        >
          {atom.text}{needsInterAtomSpacing && index < currentAtoms.length - 1 ? " " : ""}
        </span>
      {/each}
    {:else}
      {currentText}
    {/if}
  </div>
  {#if secondaryText}
    <div class="overlay-secondary" style="opacity: {Math.max(0, opacity * 0.56)}">{secondaryText}</div>
  {/if}
</section>
