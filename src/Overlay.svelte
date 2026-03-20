<script lang="ts">
  import { onMount } from "svelte";
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
  import type { OverlayRenderPayload, OverlaySettings } from "./shared/types";
  import "./overlay.css";

  /* ── State ─────────────────────────────────────────────── */

  let locked = $state(false);
  let position = $state("bottom");

  let metaText = $state("等待播放");
  let currentText = $state("等待字幕内容");
  let secondaryText = $state("");

  // CSS custom property values (applied via inline style on :root)
  let fontSize = $state(34);
  let opacity = $state(1.0);
  let color = $state("#ffffff");
  let strokeColor = $state("#000000");
  let secondaryColor = $state("#ffffff");
  let secondaryStroke = $state("#000000");

  const tauriWindow = getCurrentWindow();

  /* ── Helpers ────────────────────────────────────────────── */

  function formatDuration(ms: number): string {
    const t = Math.max(0, Math.floor(ms / 1000));
    const m = Math.floor(t / 60);
    const s = t % 60;
    return `${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
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
    currentText = payload.current?.text ?? "当前时间点暂无字幕";
    secondaryText = payload.current?.secondaryText ?? "";
    metaText = [
      payload.fileLabel ?? "未选择素材",
      payload.playback.playing ? "播放中" : "已暂停",
      formatDuration(payload.playback.currentTimeMs),
    ].join(" · ");
  }

  function clear() {
    currentText = "等待字幕内容";
    secondaryText = "";
    metaText = "等待播放";
  }

  async function applyLockState(newLocked: boolean) {
    locked = newLocked;
    await tauriWindow.setIgnoreCursorEvents(newLocked);
  }

  /* ── Drag handling ─────────────────────────────────────── */

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

  /* ── Close / lock ──────────────────────────────────────── */

  async function handleClose() {
    await emitTo("main", OVERLAY_CLOSE_EVENT, {});
    await tauriWindow.hide();
  }

  async function handleLock() {
    await applyLockState(true);
    void emitTo("main", OVERLAY_LOCK_EVENT, { locked: true });
  }

  /* ── onMount ───────────────────────────────────────────── */

  onMount(async () => {
    // Transparent background
    void getCurrentWebview().setBackgroundColor({ red: 0, green: 0, blue: 0, alpha: 0 });

    // Default unlock state
    await applyLockState(false);

    const unlistenRender = await listen<OverlayRenderPayload>(OVERLAY_RENDER_EVENT, ({ payload }) => render(payload));
    const unlistenStyle = await listen<OverlaySettings>(OVERLAY_STYLE_EVENT, ({ payload }) => applyStyle(payload));
    const unlistenClear = await listen(OVERLAY_CLEAR_EVENT, () => clear());
    const unlistenLock = await listen<{ locked: boolean }>(OVERLAY_LOCK_EVENT, ({ payload }) => {
      void applyLockState(payload.locked);
    });

    return () => {
      unlistenRender();
      unlistenStyle();
      unlistenClear();
      unlistenLock();
    };
  });
</script>

<!-- svelte-ignore a11y_no_static_element_interactions a11y_no_noninteractive_element_interactions -->
<section
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
        <rect x="3" y="11" width="18" height="11" rx="2" ry="2"/>
        <path d="M7 11V7a5 5 0 0 1 9.9-1"/>
      </svg>
      <svg class="icon-locked" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
        <rect x="3" y="11" width="18" height="11" rx="2" ry="2"/>
        <path d="M7 11V7a5 5 0 0 1 10 0v4"/>
      </svg>
    </button>
    <button class="overlay-ctrl-btn overlay-close-btn" title="隐藏悬浮窗" onclick={handleClose}>
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round">
        <line x1="18" y1="6" x2="6" y2="18"/>
        <line x1="6" y1="6" x2="18" y2="18"/>
      </svg>
    </button>
  </div>

  <div class="overlay-meta" style="opacity: {Math.max(0, opacity * 0.7)}">{metaText}</div>
  <div class="overlay-current" style="opacity: {opacity}">{currentText}</div>
  {#if secondaryText}
    <div class="overlay-secondary" style="opacity: {Math.max(0, opacity * 0.85)}">{secondaryText}</div>
  {/if}
</section>
