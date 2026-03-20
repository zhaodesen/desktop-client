import { invoke } from "@tauri-apps/api/core";
import { emitTo, listen } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { getCurrentWindow } from "@tauri-apps/api/window";
import "./overlay.css";
import {
  OVERLAY_CLEAR_EVENT,
  OVERLAY_CLOSE_EVENT,
  OVERLAY_LOCK_EVENT,
  OVERLAY_RENDER_EVENT,
  OVERLAY_STYLE_EVENT,
} from "./shared/events";
import type { OverlayRenderPayload, OverlaySettings } from "./shared/types";

function queryElement<T extends Element>(selector: string): T {
  const el = document.querySelector<T>(selector);
  if (!el) throw new Error(`找不到元素: ${selector}`);
  return el;
}

function formatDuration(ms: number): string {
  const t = Math.max(0, Math.floor(ms / 1000));
  const m = Math.floor(t / 60);
  const s = t % 60;
  return `${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
}

window.addEventListener("DOMContentLoaded", () => {
  const tauriWindow   = getCurrentWindow();
  const shell         = queryElement<HTMLElement>("#overlay-shell");
  const currentText   = queryElement<HTMLElement>("#overlay-current");
  const secondaryText = queryElement<HTMLElement>("#overlay-secondary");
  const metaText      = queryElement<HTMLElement>("#overlay-meta");
  const lockBtn       = queryElement<HTMLButtonElement>("#overlay-lock");
  const closeBtn      = queryElement<HTMLButtonElement>("#overlay-close");

  let locked = false;

  /* ── 锁定 / 解锁 ───────────────────────────────────────
   *
   * 锁定 (data-locked="true"):
   *   - setIgnoreCursorEvents(true)  → 整个窗口对鼠标透明
   *     底层应用的按钮可正常点击
   *   - 无拖拽、无关闭按钮
   *   - 锁定按钮此时已不可点（窗口忽略鼠标），视觉上保留橙色图标
   *   → 解锁只能通过主窗口的「解锁」按钮（发送 OVERLAY_LOCK_EVENT 事件）
   *     Tauri 后端事件不受 setIgnoreCursorEvents 影响，始终可接收
   *
   * 解锁 (data-locked="false"):
   *   - setIgnoreCursorEvents(false) → 可交互
   *   - 显示拖拽、关闭、锁定按钮
   * ─────────────────────────────────────────────────────── */

  /** 仅更新 UI / CSS 状态，不触发事件（防循环） */
  const applyLockState = async (newLocked: boolean) => {
    locked = newLocked;
    shell.dataset.locked = String(locked);

    // 核心：让整个窗口对鼠标事件透明（或恢复）
    await tauriWindow.setIgnoreCursorEvents(locked);
  };

  /** 由悬浮窗按钮触发（仅锁定方向），锁定后通知主窗口 */
  const lockFromOverlay = async () => {
    await applyLockState(true);
    void emitTo("main", OVERLAY_LOCK_EVENT, { locked: true });
  };

  // 悬浮窗锁定按钮：解锁状态下可见可点；锁定后不可点（但 CSS 仍显示图标提示状态）
  lockBtn.addEventListener("click", () => {
    void lockFromOverlay();
  });

  closeBtn.addEventListener("click", async () => {
    // 先通知主窗口同步设置，再直接隐藏自身
    // 用 tauriWindow.hide() 而非 invoke("hide_overlay")，避免 macOS 将焦点切换到主窗口
    await emitTo("main", OVERLAY_CLOSE_EVENT, {});
    await tauriWindow.hide();
  });

  /* ── 手动拖拽 ─────────────────────────────────────────
   *
   * 不依赖 -webkit-app-region: drag 或 data-tauri-drag-region，
   * 因为在 Tauri 2 + macOS + transparent + decorations:false 的
   * 窗口上这些方式经常失效。
   *
   * 改用 Tauri 的 window.startDragging() API：
   * 在 mousedown 时手动触发原生拖拽。
   * ─────────────────────────────────────────────────────── */

  shell.addEventListener("mousedown", (e) => {
    // 仅在解锁状态下允许拖拽
    if (locked) return;

    // 如果点击的是控制按钮，不拦截
    const target = e.target as HTMLElement;
    if (target.closest(".overlay-controls")) return;

    // 仅响应左键
    if (e.button !== 0) return;

    e.preventDefault();
    void tauriWindow.startDragging();
  });

  /* ── 阻止双击放大 ──────────────────────────────────────
   *
   * macOS 双击标题栏/拖拽区会触发窗口 zoom（最大化/还原）。
   * 在 overlay 这种无装饰的透明窗口上，这会导致窗口意外放大。
   * 通过拦截 dblclick 来阻止此行为。
   * ─────────────────────────────────────────────────────── */

  shell.addEventListener("dblclick", (e) => {
    e.preventDefault();
    e.stopPropagation();
  });

  /* ── 样式 ────────────────────────────────────────────── */

  const applyStyle = (settings: OverlaySettings) => {
    const root = document.documentElement;
    root.style.setProperty("--overlay-font-size",            `${settings.fontSize}px`);
    root.style.setProperty("--overlay-color",                settings.color);
    root.style.setProperty("--overlay-stroke-color",         settings.strokeColor);
    root.style.setProperty("--overlay-secondary-color",      settings.secondaryColor);
    root.style.setProperty("--overlay-secondary-stroke",     settings.secondaryStrokeColor);
    currentText.style.opacity   = String(settings.opacity);
    secondaryText.style.opacity = String(Math.max(0, settings.opacity * 0.85));
    metaText.style.opacity      = String(Math.max(0, settings.opacity * 0.7));
    shell.dataset.position = settings.position;
  };

  /* ── 内容渲染 ─────────────────────────────────────────── */

  const clear = () => {
    currentText.textContent   = "等待字幕内容";
    secondaryText.textContent = "";
    metaText.textContent      = "等待播放";
  };

  const render = (payload: OverlayRenderPayload) => {
    currentText.textContent   = payload.current?.text ?? "当前时间点暂无字幕";
    secondaryText.textContent = payload.current?.secondaryText ?? "";
    metaText.textContent = [
      payload.fileLabel ?? "未选择素材",
      payload.playback.playing ? "播放中" : "已暂停",
      formatDuration(payload.playback.currentTimeMs),
    ].join(" · ");
  };

  /* ── 初始化 ──────────────────────────────────────────── */

  clear();
  applyStyle({ fontSize: 34, opacity: 1.0, color: "#ffffff", position: "bottom" });
  void applyLockState(false); // 默认解锁，让用户先拖动定位

  // 强制将 WKWebView 自身背景设为透明（仅靠窗口层透明有时不够）
  void getCurrentWebview().setBackgroundColor({ red: 0, green: 0, blue: 0, alpha: 0 });

  /* ── 后端 / 跨窗口事件 ───────────────────────────────── */

  void listen<OverlayRenderPayload>(OVERLAY_RENDER_EVENT, ({ payload }) => render(payload));
  void listen<OverlaySettings>(OVERLAY_STYLE_EVENT, ({ payload }) => applyStyle(payload));
  void listen(OVERLAY_CLEAR_EVENT, () => clear());

  // 主窗口发来的锁定/解锁指令（Tauri 事件不受 setIgnoreCursorEvents 影响）
  void listen<{ locked: boolean }>(OVERLAY_LOCK_EVENT, ({ payload }) => {
    void applyLockState(payload.locked);
    // 不再 emit 回去，主窗口已知晓（它自己触发的）
  });
});
