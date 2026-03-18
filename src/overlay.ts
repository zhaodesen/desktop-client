import { listen } from "@tauri-apps/api/event";
import "./overlay.css";
import {
  OVERLAY_CLEAR_EVENT,
  OVERLAY_RENDER_EVENT,
  OVERLAY_STYLE_EVENT,
} from "./shared/events";
import type { OverlayRenderPayload, OverlaySettings } from "./shared/types";

function queryElement<T extends Element>(selector: string): T {
  const element = document.querySelector<T>(selector);
  if (!element) {
    throw new Error(`找不到元素: ${selector}`);
  }
  return element;
}

function formatDuration(ms: number): string {
  const totalSeconds = Math.max(0, Math.floor(ms / 1000));
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  return `${String(minutes).padStart(2, "0")}:${String(seconds).padStart(2, "0")}`;
}

window.addEventListener("DOMContentLoaded", () => {
  const shell = queryElement<HTMLElement>("#overlay-shell");
  const currentText = queryElement<HTMLElement>("#overlay-current");
  const metaText = queryElement<HTMLElement>("#overlay-meta");

  const applyStyle = (settings: OverlaySettings) => {
    document.documentElement.style.setProperty("--overlay-font-size", `${settings.fontSize}px`);
    document.documentElement.style.setProperty("--overlay-color", settings.color);
    shell.style.opacity = String(settings.opacity);
    shell.dataset.position = settings.position;
  };

  const clear = () => {
    currentText.textContent = "等待字幕内容";
    metaText.textContent = "等待播放";
    shell.dataset.active = "false";
  };

  const render = (payload: OverlayRenderPayload) => {
    currentText.textContent = payload.current?.text ?? "当前时间点暂无字幕";
    metaText.textContent = [
      payload.fileLabel ?? "未选择素材",
      payload.playback.playing ? "播放中" : "已暂停",
      formatDuration(payload.playback.currentTimeMs),
    ].join(" · ");
    shell.dataset.active = String(Boolean(payload.current));
  };

  clear();
  applyStyle({
    fontSize: 34,
    opacity: 0.92,
    color: "#fff4d6",
    position: "bottom",
  });

  void listen<OverlayRenderPayload>(OVERLAY_RENDER_EVENT, ({ payload }) => {
    render(payload);
  });
  void listen<OverlaySettings>(OVERLAY_STYLE_EVENT, ({ payload }) => {
    applyStyle(payload);
  });
  void listen(OVERLAY_CLEAR_EVENT, () => {
    clear();
  });
});
