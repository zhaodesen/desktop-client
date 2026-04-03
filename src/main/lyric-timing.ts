import type {
  PlaybackClockAnchor,
  PlaybackSnapshot,
  SubtitleAtom,
  SubtitleCue,
  SubtitleDisplayMode,
} from "../shared/types";

export type DisplaySubtitleCue = Omit<SubtitleCue, "text" | "secondaryText" | "atoms"> & {
  text: string;
  secondaryText?: string;
  atoms: SubtitleAtom[];
};

export function createPlaybackClockAnchor(
  snapshot: PlaybackSnapshot,
  wallTimeMs = Date.now(),
): PlaybackClockAnchor {
  return {
    mediaTimeMs: Math.max(0, snapshot.currentTimeMs),
    wallTimeMs,
    durationMs: Math.max(0, snapshot.durationMs),
    rate: snapshot.rate,
    playing: snapshot.playing,
  };
}

export function predictPlaybackTime(
  anchor: PlaybackClockAnchor,
  wallTimeMs = Date.now(),
): number {
  if (!anchor.playing) {
    return clampPlaybackTime(anchor.mediaTimeMs, anchor.durationMs);
  }

  const elapsedMs = Math.max(0, wallTimeMs - anchor.wallTimeMs);
  return clampPlaybackTime(anchor.mediaTimeMs + elapsedMs * anchor.rate, anchor.durationMs);
}

function clampPlaybackTime(timeMs: number, durationMs: number): number {
  if (durationMs <= 0) {
    return Math.max(0, timeMs);
  }
  return Math.max(0, Math.min(durationMs, timeMs));
}

export function findCueIndexAtTime(cues: SubtitleCue[], timeMs: number): number {
  let low = 0;
  let high = cues.length - 1;

  while (low <= high) {
    const mid = (low + high) >>> 1;
    const cue = cues[mid];
    if (timeMs < cue.startMs) {
      high = mid - 1;
    } else if (timeMs >= cue.endMs) {
      low = mid + 1;
    } else {
      return mid;
    }
  }

  return -1;
}

export function normalizeSubtitleText(value: string): string {
  return value.trim().replace(/\s+/g, " ");
}

export function countVisibleChars(value: string): number {
  return Array.from(value).filter((char) => !/\s/.test(char)).length;
}

export function containsCjk(value: string): boolean {
  return /[\u4e00-\u9fff\u3040-\u30ff\uac00-\ud7af]/.test(value);
}

export function isSplitPunctuation(char: string): boolean {
  return /[,.!?;:，。！？；：、]/.test(char);
}

export function splitTextUnits(value: string): string[] {
  const words = value.trim().split(/\s+/).filter(Boolean);
  if (words.length > 1) {
    return words;
  }

  const units: string[] = [];
  for (const char of Array.from(value)) {
    if (/\s/.test(char)) continue;
    if (isSplitPunctuation(char) && units.length > 0) {
      units[units.length - 1] += char;
    } else {
      units.push(char);
    }
  }
  return units;
}

export function cueNeedsInterAtomSpacing(text: string): boolean {
  const normalized = normalizeSubtitleText(text);
  return normalized.includes(" ") && splitTextUnits(normalized).length > 1;
}

export function normalizeAtoms(
  atoms: SubtitleAtom[] | undefined,
  cueStartMs: number,
  cueEndMs: number,
): SubtitleAtom[] {
  return [...(atoms ?? [])]
    .map((atom) => ({
      text: normalizeSubtitleText(atom.text),
      startMs: Math.max(cueStartMs, Math.min(cueEndMs, atom.startMs)),
      endMs: Math.max(cueStartMs, Math.min(cueEndMs, atom.endMs)),
    }))
    .filter((atom) => atom.text.length > 0)
    .sort((left, right) => left.startMs - right.startMs)
    .map((atom) => ({
      ...atom,
      endMs: Math.max(atom.startMs, atom.endMs),
    }));
}

export function atomsMatchText(atoms: SubtitleAtom[], text: string): boolean {
  return atoms.map((atom) => normalizeSubtitleText(atom.text)).join("\n")
    === splitTextUnits(text).join("\n");
}

export function buildFallbackAtoms(
  text: string,
  startMs: number,
  endMs: number,
): SubtitleAtom[] {
  const normalizedText = normalizeSubtitleText(text);
  const units = splitTextUnits(normalizedText);
  if (units.length === 0) {
    return [];
  }

  if (units.length === 1) {
    return [{ text: normalizedText, startMs, endMs }];
  }

  const weights = units.map((unit) => Math.max(1, countVisibleChars(unit)));
  const totalWeight = weights.reduce((sum, value) => sum + value, 0) || 1;
  const duration = Math.max(0, endMs - startMs);
  let remainingDuration = duration;
  let remainingWeight = totalWeight;
  let cursor = startMs;

  return units.map((unit, index) => {
    const isLast = index === units.length - 1;
    const atomEnd = isLast
      ? endMs
      : (() => {
          const remainingUnits = units.length - index - 1;
          const idealDuration = Math.round((remainingDuration * weights[index]) / remainingWeight);
          const minDuration = remainingDuration > 0
            ? Math.max(1, Math.floor(remainingDuration / (remainingUnits + 1)))
            : 0;
          const maxDuration = Math.max(
            minDuration,
            remainingDuration - minDuration * remainingUnits,
          );
          return cursor + Math.min(maxDuration, Math.max(minDuration, idealDuration));
        })();

    const atom = {
      text: unit,
      startMs: cursor,
      endMs: atomEnd,
    };

    remainingDuration = Math.max(0, remainingDuration - (atomEnd - cursor));
    remainingWeight = Math.max(0, remainingWeight - weights[index]);
    cursor = atomEnd;
    return atom;
  });
}

export function fitAtomsToText(
  text: string,
  cueStartMs: number,
  cueEndMs: number,
  sourceAtoms: SubtitleAtom[] | undefined,
): SubtitleAtom[] {
  const normalizedSource = normalizeAtoms(sourceAtoms, cueStartMs, cueEndMs);
  if (normalizedSource.length === 0) {
    return buildFallbackAtoms(text, cueStartMs, cueEndMs);
  }

  if (atomsMatchText(normalizedSource, text)) {
    return normalizedSource;
  }

  const units = splitTextUnits(text);
  if (units.length === normalizedSource.length) {
    return normalizedSource.map((atom, index) => ({
      text: units[index] ?? atom.text,
      startMs: atom.startMs,
      endMs: atom.endMs,
    }));
  }

  return buildFallbackAtoms(text, cueStartMs, cueEndMs);
}

export function ensureCueAtoms(cue: SubtitleCue): SubtitleAtom[] {
  const normalizedAtoms = normalizeAtoms(cue.atoms, cue.startMs, cue.endMs);
  if (normalizedAtoms.length > 0 && atomsMatchText(normalizedAtoms, cue.text)) {
    return normalizedAtoms;
  }
  return buildFallbackAtoms(cue.text, cue.startMs, cue.endMs);
}

export function buildDisplayCue(
  cue: SubtitleCue | undefined,
  mode: SubtitleDisplayMode,
): DisplaySubtitleCue | undefined {
  if (!cue) return undefined;

  const baseAtoms = ensureCueAtoms(cue);
  if (mode === "original") {
    return {
      ...cue,
      text: cue.text,
      secondaryText: undefined,
      atoms: baseAtoms,
    };
  }

  if (mode === "translation") {
    const translatedText = cue.secondaryText?.trim() ?? "";
    return {
      ...cue,
      text: translatedText,
      secondaryText: undefined,
      atoms: fitAtomsToText(translatedText, cue.startMs, cue.endMs, baseAtoms),
    };
  }

  return {
    ...cue,
    text: cue.text,
    secondaryText: cue.secondaryText,
    atoms: baseAtoms,
  };
}

export function getAtomState(
  atom: SubtitleAtom,
  timeMs: number,
): "past" | "active" | "future" {
  if (timeMs >= atom.endMs) return "past";
  if (timeMs >= atom.startMs) return "active";
  return "future";
}

export function getActiveAtomIndex(atoms: SubtitleAtom[], timeMs: number): number {
  return atoms.findIndex((atom) => timeMs >= atom.startMs && timeMs < atom.endMs);
}
