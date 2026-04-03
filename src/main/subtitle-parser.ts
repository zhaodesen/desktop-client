import type { SubtitleCue } from "../shared/types";
import {
  buildFallbackAtoms,
  countVisibleChars,
  containsCjk,
  fitAtomsToText,
  normalizeSubtitleText,
  splitTextUnits,
} from "./lyric-timing";

type JsonSubtitleDocument = {
  version?: number;
  cues?: SubtitleCue[];
};

const MAX_SEGMENT_DURATION_MS = 2600;
const MIN_SEGMENT_DURATION_MS = 700;
const MAX_LATIN_SEGMENT_CHARS = 26;
const MAX_CJK_SEGMENT_CHARS = 16;
const MIN_SPLIT_VISIBLE_CHARS = 12;

function parseTimestamp(token: string): number {
  const normalized = token.trim().replace(",", ".");
  const parts = normalized.split(":");

  if (parts.length < 2 || parts.length > 3) {
    throw new Error(`非法时间戳: ${token}`);
  }

  const [hours, minutes, secondsWithMillis] =
    parts.length === 3
      ? [Number(parts[0]), Number(parts[1]), parts[2]]
      : [0, Number(parts[0]), parts[1]];

  const [secondsPart, millisPart = "0"] = secondsWithMillis.split(".");
  const seconds = Number(secondsPart);
  const millis = Number((millisPart + "000").slice(0, 3));

  if ([hours, minutes, seconds, millis].some(Number.isNaN)) {
    throw new Error(`非法时间戳: ${token}`);
  }

  return (((hours * 60 + minutes) * 60 + seconds) * 1000) + millis;
}

function parseCue(block: string, fallbackId: number): SubtitleCue | null {
  const lines = block
    .split("\n")
    .map((line) => line.trimEnd())
    .filter((line, index, all) => line.length > 0 || index < all.length - 1);

  if (lines.length === 0) {
    return null;
  }

  const marker = lines[0].trim();
  if (
    marker.startsWith("NOTE") ||
    marker.startsWith("STYLE") ||
    marker.startsWith("REGION")
  ) {
    return null;
  }

  const timelineIndex = lines.findIndex((line) => line.includes("-->"));
  if (timelineIndex === -1) {
    return null;
  }

  const timeline = lines[timelineIndex];
  const [rawStart, rawEndWithSettings] = timeline.split("-->");

  if (!rawStart || !rawEndWithSettings) {
    return null;
  }

  const startMs = parseTimestamp(rawStart);
  const endMs = parseTimestamp(rawEndWithSettings.trim().split(/\s+/)[0]);
  const contentLines = lines
    .slice(timelineIndex + 1)
    .map((line) => line.trim())
    .filter((line) => line.length > 0);
  if (contentLines.length === 0) {
    return null;
  }

  const lastLine = contentLines[contentLines.length - 1];
  const hasSecondaryText = contentLines.length > 1 && containsChinese(lastLine);
  const text = hasSecondaryText
    ? contentLines.slice(0, -1).join("\n").trim()
    : contentLines.join("\n").trim();
  const secondaryText = hasSecondaryText ? lastLine.trim() : undefined;

  if (!text) return null;

  return {
    id: fallbackId,
    startMs,
    endMs,
    text,
    secondaryText,
    atoms: [],
  };
}

function containsChinese(value: string): boolean {
  return /[\u4e00-\u9fff]/.test(value);
}

function estimateSegmentCount(text: string, durationMs: number): number {
  if (durationMs < MIN_SEGMENT_DURATION_MS * 2) {
    return 1;
  }

  const visibleChars = countVisibleChars(text);
  if (visibleChars < MIN_SPLIT_VISIBLE_CHARS) {
    return 1;
  }

  const maxChars = containsCjk(text) ? MAX_CJK_SEGMENT_CHARS : MAX_LATIN_SEGMENT_CHARS;
  const durationSegments = Math.ceil(durationMs / MAX_SEGMENT_DURATION_MS);
  const lengthSegments = Math.ceil(visibleChars / maxChars);
  const maxSegments = Math.max(1, Math.floor(durationMs / MIN_SEGMENT_DURATION_MS));

  return Math.min(maxSegments, Math.max(durationSegments, lengthSegments));
}

function joinTextUnits(units: string[]): string {
  if (
    units.every((unit) => {
      const chars = Array.from(unit);
      const lastChar = chars[chars.length - 1];
      return chars.length === 1 || /[,.!?;:，。！？；：、]/.test(lastChar ?? "");
    })
  ) {
    return units.join("");
  }
  return units.join(" ");
}

function splitTextIntoSegments(value: string, requestedSegments: number): string[] {
  const units = splitTextUnits(value);
  if (units.length <= 1 || requestedSegments <= 1) {
    return [normalizeSubtitleText(value)];
  }

  const segmentCount = Math.min(requestedSegments, units.length);
  const weights = units.map((unit) => Math.max(1, countVisibleChars(unit)));
  let remainingWeight = weights.reduce((sum, weight) => sum + weight, 0);
  let cursor = 0;
  const segments: string[] = [];

  for (let segmentIndex = 0; segmentIndex < segmentCount; segmentIndex += 1) {
    const remainingSegments = segmentCount - segmentIndex;
    const remainingUnits = units.length - cursor;

    if (remainingSegments === 1) {
      segments.push(joinTextUnits(units.slice(cursor)));
      break;
    }

    const targetWeight = remainingWeight / remainingSegments;
    let currentWeight = 0;
    let end = cursor;

    while (end < units.length) {
      const unitsLeftAfterPick = units.length - (end + 1);
      if (unitsLeftAfterPick < remainingSegments - 1) {
        break;
      }

      const nextWeight = currentWeight + weights[end];
      const shouldStop =
        end > cursor &&
        Math.abs(nextWeight - targetWeight) > Math.abs(currentWeight - targetWeight);
      if (shouldStop) {
        break;
      }

      currentWeight = nextWeight;
      end += 1;

      if (currentWeight >= targetWeight && remainingUnits > remainingSegments) {
        break;
      }
    }

    if (end === cursor) {
      end += 1;
      currentWeight = weights[cursor];
    }

    segments.push(joinTextUnits(units.slice(cursor, end)));
    cursor = end;
    remainingWeight -= currentWeight;
  }

  return segments
    .map((segment) => normalizeSubtitleText(segment))
    .filter(Boolean);
}

function splitCue(cue: SubtitleCue): SubtitleCue[] {
  const text = normalizeSubtitleText(cue.text);
  if (!text) return [];

  const secondaryText = cue.secondaryText ? normalizeSubtitleText(cue.secondaryText) : undefined;
  const hasTimedAtoms = Array.isArray(cue.atoms) && cue.atoms.length > 0;
  if (hasTimedAtoms) {
    return [{
      ...cue,
      text,
      secondaryText,
      atoms: fitAtomsToText(text, cue.startMs, cue.endMs, cue.atoms),
    }];
  }

  const duration = Math.max(0, cue.endMs - cue.startMs);
  const segmentCount = estimateSegmentCount(text, duration);
  if (segmentCount <= 1) {
    return [{ ...cue, text, secondaryText, atoms: buildFallbackAtoms(text, cue.startMs, cue.endMs) }];
  }

  const primarySegments = splitTextIntoSegments(text, segmentCount);
  if (primarySegments.length <= 1) {
    return [{ ...cue, text, secondaryText, atoms: buildFallbackAtoms(text, cue.startMs, cue.endMs) }];
  }

  const secondarySegments = secondaryText
    ? splitTextIntoSegments(secondaryText, primarySegments.length)
    : undefined;
  const weights = primarySegments.map((segment) => Math.max(1, countVisibleChars(segment)));
  const totalWeight = weights.reduce((sum, weight) => sum + weight, 0) || 1;
  let remainingDuration = duration;
  let remainingWeight = totalWeight;
  let segmentStart = cue.startMs;

  return primarySegments.map((segmentText, index) => {
    const isLast = index === primarySegments.length - 1;
    const segmentEnd = isLast
      ? cue.endMs
      : (() => {
          const remainingSegments = primarySegments.length - index;
          const idealDuration = Math.round((remainingDuration * weights[index]) / remainingWeight);
          const minDuration = MIN_SEGMENT_DURATION_MS;
          const maxDuration = Math.max(
            minDuration,
            remainingDuration - minDuration * (remainingSegments - 1),
          );
          const segmentDuration = Math.min(
            maxDuration,
            Math.max(minDuration, idealDuration),
          );
          return segmentStart + segmentDuration;
        })();

    const nextCue: SubtitleCue = {
      id: cue.id,
      startMs: segmentStart,
      endMs: segmentEnd,
      text: segmentText,
      secondaryText: secondarySegments?.[index] || undefined,
      atoms: buildFallbackAtoms(segmentText, segmentStart, segmentEnd),
    };

    remainingDuration = Math.max(0, remainingDuration - (segmentEnd - segmentStart));
    remainingWeight = Math.max(0, remainingWeight - weights[index]);
    segmentStart = segmentEnd;
    return nextCue;
  });
}

function parseJsonSubtitle(content: string): SubtitleCue[] | null {
  try {
    const parsed = JSON.parse(content) as JsonSubtitleDocument | SubtitleCue[];
    const cues = Array.isArray(parsed) ? parsed : parsed.cues;
    if (!Array.isArray(cues)) return null;
    return cues
      .filter((cue) => cue && typeof cue.text === "string")
      .map((cue, index) => ({
        id: cue.id ?? index + 1,
        startMs: cue.startMs,
        endMs: cue.endMs,
        text: cue.text,
        secondaryText: cue.secondaryText,
        atoms: Array.isArray(cue.atoms) ? cue.atoms : [],
      }))
      .flatMap((cue) => splitCue(cue))
      .map((cue, index) => ({ ...cue, id: index + 1 }));
  } catch {
    return null;
  }
}

export function parseSubtitleText(content: string): SubtitleCue[] {
  const jsonCues = parseJsonSubtitle(content);
  if (jsonCues) {
    return jsonCues;
  }

  const normalized = content
    .replace(/^\uFEFF/, "")
    .replace(/\r/g, "")
    .trim();

  if (!normalized) {
    return [];
  }

  const withoutHeader = normalized.replace(/^WEBVTT[^\n]*\n+/i, "");
  const blocks = withoutHeader.split(/\n{2,}/);

  const cues: SubtitleCue[] = [];

  for (const block of blocks) {
    const cue = parseCue(block, cues.length + 1);
    if (cue) {
      cues.push(...splitCue(cue));
    }
  }

  return cues.map((cue, index) => ({ ...cue, id: index + 1 }));
}
