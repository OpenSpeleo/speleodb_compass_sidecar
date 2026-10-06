import { describe, expect, it } from "vitest";
import vectors from "./__fixtures__/import-selection.json";
import {
  allSectionIds,
  resolveSelection,
  sectionMatches,
  toggleSection,
} from "./import-selection";
import type { ImportPreview, ImportSection } from "./types";

const preview = (): ImportPreview => ({
  preview_id: "preview",
  source_name: "Cave.mak",
  source_directory: "Surveys/Cave",
  full_import_reason: null,
  sections: [0, 1, 2].map((id) => ({
    id,
    name: `${String.fromCharCode(65 + id)}.DAT`,
    relative_path: `region/${id}.DAT`,
    dependencies:
      id === 1 ? [{ section_id: 0, reason: "Connects at station A1" }] : [],
  })),
});

describe("Rust selection conformance", () => {
  for (const vector of vectors)
    it(vector.name, () => {
      if ("error" in vector.result) {
        expect(() =>
          resolveSelection(vector.sections, new Set(vector.explicit)),
        ).toThrow(vector.result.error);
      } else {
        const result = resolveSelection(
          vector.sections,
          new Set(vector.explicit),
        );
        expect([...result.included]).toEqual(vector.result.included);
        expect(
          Object.fromEntries(
            [...result.requiredBy].map(([id, roots]) => [id, [...roots]]),
          ),
        ).toEqual(vector.result.requiredBy);
      }
    });
  it("handles the backend 500-section dense graph across bitset word boundaries", () => {
    const sections: ImportSection[] = Array.from({ length: 500 }, (_, id) => ({
      id,
      name: `${id}.DAT`,
      relative_path: `${id}.DAT`,
      dependencies: Array.from({ length: id }, (_, section_id) => ({
        section_id,
        reason: "Shared station",
      })),
    }));
    const all = new Set(sections.map((section) => section.id));
    const selected = resolveSelection(sections, all);
    expect(selected.included).toEqual(all);
    expect(selected.requiredBy.get(0)?.size).toBe(499);
    expect(selected.requiredBy.get(498)).toEqual(new Set([499]));
    expect(selected.requiredBy.has(499)).toBe(false);
    const last = resolveSelection(sections, new Set([499]));
    expect(last.included.size).toBe(500);
    expect(last.requiredBy.get(0)).toEqual(new Set([499]));
  });
});

describe("selection interactions", () => {
  it("preserves explicit prerequisites when dependents are removed", () => {
    const value = preview();
    const all = allSectionIds(value);
    expect(toggleSection(value, all, 0)).toEqual(all);
    const withoutDependent = toggleSection(value, all, 1);
    expect(withoutDependent).toEqual(new Set([0, 2]));
    expect(toggleSection(value, withoutDependent, 0)).toEqual(new Set([2]));
  });
  it("adds only explicit roots and removes automatic dependencies with their root", () => {
    const value = preview();
    const explicit = toggleSection(value, new Set(), 1);
    expect(explicit).toEqual(new Set([1]));
    expect(resolveSelection(value.sections, explicit).included).toEqual(
      new Set([0, 1]),
    );
    expect(toggleSection(value, explicit, 1).size).toBe(0);
  });
  it("matches trimmed case-insensitive names and paths", () => {
    const section = preview().sections[0]!;
    expect(sectionMatches(section, " a.dat ")).toBe(true);
    expect(sectionMatches(section, "REGION/0")).toBe(true);
    expect(sectionMatches(section, "B.DAT")).toBe(false);
    expect(sectionMatches(section, "\u0085A.DAT\u0085")).toBe(true);
    expect(sectionMatches(section, "\uFEFFA.DAT")).toBe(false);
  });
  it("cannot toggle full-only previews or unknown IDs", () => {
    const value = preview();
    const all = allSectionIds(value);
    expect(toggleSection(value, all, 300)).toEqual(all);
    value.full_import_reason = "Unsupported directive";
    expect(toggleSection(value, all, 2)).toEqual(all);
  });
});
