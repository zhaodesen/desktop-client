import type { SubtitleContext, SubtitleCue } from "../shared/types";

export class SubtitleEngine {
  private cues: SubtitleCue[] = [];

  load(cues: SubtitleCue[]): void {
    this.cues = [...cues].sort((left, right) => left.startMs - right.startMs);
  }

  clear(): void {
    this.cues = [];
  }

  size(): number {
    return this.cues.length;
  }

  getContext(currentTimeMs: number): SubtitleContext {
    if (this.cues.length === 0) {
      return {};
    }

    const currentIndex = this.findCurrentIndex(currentTimeMs);
    if (currentIndex >= 0) {
      return {
        previous: this.cues[currentIndex - 1],
        current: this.cues[currentIndex],
        next: this.cues[currentIndex + 1],
      };
    }

    const nextIndex = this.findNextIndex(currentTimeMs);
    if (nextIndex === -1) {
      return {
        previous: this.cues[this.cues.length - 1],
      };
    }

    return {
      previous: this.cues[nextIndex - 1],
      next: this.cues[nextIndex],
    };
  }

  private findCurrentIndex(currentTimeMs: number): number {
    let low = 0;
    let high = this.cues.length - 1;

    while (low <= high) {
      const mid = Math.floor((low + high) / 2);
      const cue = this.cues[mid];

      if (currentTimeMs < cue.startMs) {
        high = mid - 1;
      } else if (currentTimeMs > cue.endMs) {
        low = mid + 1;
      } else {
        return mid;
      }
    }

    return -1;
  }

  private findNextIndex(currentTimeMs: number): number {
    let low = 0;
    let high = this.cues.length;

    while (low < high) {
      const mid = Math.floor((low + high) / 2);
      if (this.cues[mid].startMs <= currentTimeMs) {
        low = mid + 1;
      } else {
        high = mid;
      }
    }

    return low >= this.cues.length ? -1 : low;
  }
}
