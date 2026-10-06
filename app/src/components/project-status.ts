import type { LocalProjectStatus } from "../lib/types";
import {
  COLOR_ALARM,
  COLOR_GOOD,
  COLOR_WARN,
  FONT_COLOR_BLUE,
} from "../lib/constants";

export function projectStatusPresentation(status: LocalProjectStatus) {
  switch (status) {
    case "UpToDate":
      return {
        icon: "FONT_AWESOME_SOLID_FILE_CIRCLE_CHECK",
        color: COLOR_GOOD,
        text: "Up to Date",
      } as const;
    case "Dirty":
      return {
        icon: "FONT_AWESOME_SOLID_FILE_CIRCLE_EXCLAMATION",
        color: COLOR_WARN,
        text: "Unsaved Local Changes",
      } as const;
    case "RemoteOnly":
      return {
        icon: "FONT_AWESOME_SOLID_FILE_ARROW_DOWN",
        color: COLOR_GOOD,
        text: "Available for Download",
      } as const;
    case "Unknown":
      return {
        icon: "FONT_AWESOME_SOLID_FILE_CIRCLE_CHECK",
        color: COLOR_GOOD,
        text: "Unknown",
      } as const;
    case "EmptyLocal":
      return {
        icon: "FONT_AWESOME_SOLID_FILE_CIRCLE_PLUS",
        color: COLOR_GOOD,
        text: "Empty Project",
      } as const;
    case "OutOfDate":
      return {
        icon: "FONT_AWESOME_SOLID_FILE_ARROW_DOWN",
        color: FONT_COLOR_BLUE,
        text: "Update Available",
      } as const;
    case "DirtyAndOutOfDate":
      return {
        icon: "FONT_AWESOME_SOLID_FACE_SAD_CRY",
        color: COLOR_ALARM,
        text: "Local Changes & Update Available",
      } as const;
  }
}

/** Rust strings compare by Unicode scalar order, independent of the host locale. */
export function compareUnicode(a: string, b: string): number {
  const left = Array.from(a),
    right = Array.from(b);
  for (let i = 0; i < Math.min(left.length, right.length); i++) {
    const delta = left[i]!.codePointAt(0)! - right[i]!.codePointAt(0)!;
    if (delta !== 0) return delta;
  }
  return left.length - right.length;
}
