import type { PlaybackSnapshot, SubtitleCue } from "../shared/types";
import { LoopController, type LoopMode } from "./loop-controller";

type SnapshotListener = (snapshot: PlaybackSnapshot) => void;
type EndedListener = () => void;

export class PlayerController {
  private readonly audio: HTMLAudioElement;
  private readonly listeners = new Set<SnapshotListener>();
  private readonly endedListeners = new Set<EndedListener>();
  private readonly loopController = new LoopController();
  private objectUrl?: string;
  private tickerId?: number;

  constructor(audio: HTMLAudioElement) {
    this.audio = audio;

    this.audio.addEventListener("play", () => {
      this.startTicker();
      this.publish();
    });
    this.audio.addEventListener("pause", () => {
      this.stopTicker();
      this.publish();
    });
    this.audio.addEventListener("loadedmetadata", () => this.publish());
    this.audio.addEventListener("ratechange", () => this.publish());
    this.audio.addEventListener("seeked", () => this.publish());
    this.audio.addEventListener("timeupdate", () => this.publish());
    this.audio.addEventListener("ended", () => {
      this.stopTicker();
      this.publish();
      for (const listener of this.endedListeners) {
        listener();
      }
    });
  }

  subscribe(listener: SnapshotListener): () => void {
    this.listeners.add(listener);
    listener(this.getSnapshot());

    return () => {
      this.listeners.delete(listener);
    };
  }

  async loadFile(file: File): Promise<void> {
    if (this.objectUrl) {
      URL.revokeObjectURL(this.objectUrl);
    }

    this.objectUrl = URL.createObjectURL(file);
    this.audio.src = this.objectUrl;
    this.audio.load();

    await new Promise<void>((resolve, reject) => {
      const cleanup = () => {
        this.audio.removeEventListener("loadedmetadata", handleLoadedMetadata);
        this.audio.removeEventListener("error", handleError);
      };

      const handleLoadedMetadata = () => {
        cleanup();
        resolve();
      };

      const handleError = () => {
        cleanup();
        reject(new Error("媒体文件加载失败"));
      };

      this.audio.addEventListener("loadedmetadata", handleLoadedMetadata);
      this.audio.addEventListener("error", handleError);
    });

    this.publish();
  }

  async loadUrl(url: string): Promise<void> {
    if (this.objectUrl) {
      URL.revokeObjectURL(this.objectUrl);
      this.objectUrl = undefined;
    }

    this.audio.src = url;
    this.audio.load();

    await new Promise<void>((resolve, reject) => {
      const cleanup = () => {
        this.audio.removeEventListener("loadedmetadata", handleLoadedMetadata);
        this.audio.removeEventListener("error", handleError);
      };

      const handleLoadedMetadata = () => {
        cleanup();
        resolve();
      };

      const handleError = () => {
        cleanup();
        reject(new Error("媒体文件加载失败"));
      };

      this.audio.addEventListener("loadedmetadata", handleLoadedMetadata);
      this.audio.addEventListener("error", handleError);
    });

    this.publish();
  }

  onEnded(listener: EndedListener): () => void {
    this.endedListeners.add(listener);
    return () => {
      this.endedListeners.delete(listener);
    };
  }

  hasMedia(): boolean {
    return this.audio.src.length > 0;
  }

  getSnapshot(): PlaybackSnapshot {
    const duration = Number.isFinite(this.audio.duration)
      ? this.audio.duration * 1000
      : 0;

    return {
      playing: !this.audio.paused,
      currentTimeMs: this.audio.currentTime * 1000,
      durationMs: duration,
      rate: this.audio.playbackRate,
    };
  }

  getLoopMode(): LoopMode {
    return this.loopController.getMode();
  }

  clearLoop(): void {
    this.loopController.clear();
    this.publish();
  }

  loopCurrentCue(cue: SubtitleCue): void {
    this.loopController.setSingleCue(cue);
    this.publish();
  }

  async togglePlayback(): Promise<void> {
    if (this.audio.paused) {
      await this.audio.play();
      return;
    }

    this.audio.pause();
  }

  pause(): void {
    this.audio.pause();
  }

  seek(currentTimeMs: number): void {
    this.audio.currentTime = Math.max(currentTimeMs, 0) / 1000;
    this.publish();
  }

  setPlaybackRate(rate: number): void {
    this.audio.playbackRate = rate;
    this.publish();
  }

  private startTicker(): void {
    if (this.tickerId !== undefined) {
      return;
    }

    this.tickerId = window.setInterval(() => {
      const seekTarget = this.loopController.getSeekTarget(
        this.audio.currentTime * 1000,
      );

      if (seekTarget !== null) {
        this.audio.currentTime = seekTarget / 1000;
      }

      this.publish();
    }, 80);
  }

  private stopTicker(): void {
    if (this.tickerId === undefined) {
      return;
    }

    window.clearInterval(this.tickerId);
    this.tickerId = undefined;
  }

  private publish(): void {
    const snapshot = this.getSnapshot();
    for (const listener of this.listeners) {
      listener(snapshot);
    }
  }
}
