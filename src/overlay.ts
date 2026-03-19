import { invoke } from "@tauri-apps/api/core";
import { emitTo, listen } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { getCurrentWindow } from "@tauri-apps/api/window";
import "./overlay.css";
import {
  OVERLAY_CLEAR_EVENT,
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
  const tauriWindow = getCurrentWindow();
  const shell       = queryElement<HTMLElement>("#overlay-shell");
  const currentText = queryElement<HTMLElement>("#overlay-current");
  const metaText    = queryElement<HTMLElement>("#overlay-meta");
  const lockBtn     = queryElement<HTMLButtonElement>("#overlay-lock");
  const closeBtn    = queryElement<HTMLButtonElement>("#overlay-close");

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
  const applyLockState = async (locked: boolean) => {
    shell.dataset.locked = String(locked);

    if (locked) {
      shell.removeAttribute("data-tauri-drag-region");
    } else {
      shell.setAttribute("data-tauri-drag-region", "");
    }

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

  closeBtn.addEventListener("click", () => {
    void invoke("hide_overlay");
  });

  /* ── 样式 ────────────────────────────────────────────── */

  const applyStyle = (settings: OverlaySettings) => {
    document.documentElement.style.setProperty("--overlay-font-size", `${settings.fontSize}px`);
    document.documentElement.style.setProperty("--overlay-color", settings.color);
    currentText.style.opacity = String(settings.opacity);
    metaText.style.opacity = String(Math.max(0, settings.opacity * 0.7));
    shell.dataset.position = settings.position;
  };

  /* ── 内容渲染 ─────────────────────────────────────────── */

  const clear = () => {
    currentText.textContent = "等待字幕内容";
    metaText.textContent = "等待播放";
  };

  const render = (payload: OverlayRenderPayload) => {
    currentText.textContent = payload.current?.text ?? "当前时间点暂无字幕";
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
