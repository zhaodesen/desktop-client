import type { SubtitleCue } from "../shared/types";

export type LoopMode =
  | { kind: "off" }
  | {
      kind: "single-cue";
      cueId: number;
      startMs: number;
      endMs: number;
    }
  | {
      kind: "range";
      startMs: number;
      endMs: number;
    };

const LOOP_MARGIN_MS = 60;

export class LoopController {
  private mode: LoopMode = { kind: "off" };

  getMode(): LoopMode {
    return this.mode;
  }

  clear(): void {
    this.mode = { kind: "off" };
  }

  setSingleCue(cue: SubtitleCue): void {
    this.mode = {
      kind: "single-cue",
      cueId: cue.id,
      startMs: cue.startMs,
      endMs: cue.endMs,
    };
  }

  setRange(startMs: number, endMs: number): void {
    this.mode = {
      kind: "range",
      startMs,
      endMs,
    };
  }

  getSeekTarget(currentTimeMs: number): number | null {
    if (this.mode.kind === "off") {
      return null;
    }

    if (currentTimeMs >= this.mode.endMs - LOOP_MARGIN_MS) {
      return this.mode.startMs;
    }

    return null;
  }
}
