import type { SubtitleCue } from "../shared/types";

type JsonSubtitleDocument = {
  cues?: SubtitleCue[];
};

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
  };
}

function containsChinese(value: string): boolean {
  return /[\u4e00-\u9fff]/.test(value);
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
      }));
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
      cues.push(cue);
    }
  }

  return cues;
}
