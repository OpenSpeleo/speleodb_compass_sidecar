import type { ImportPreview, ImportSection } from "./types";
import { rustTrim } from "./text";

export interface ImportSelection {
  included: Set<number>;
  requiredBy: Map<number, Set<number>>;
}

/** The frontend projection of common::compass_import::resolve_selection.
 * Propagate root bitsets once through the backwards DAG, rather than walking
 * the graph separately for every selected root. Rust remains authoritative.
 */
export function resolveSelection(
  sections: ImportSection[],
  explicit: Set<number>,
): ImportSelection {
  const byId = new Map(sections.map((section) => [section.id, section]));
  if (
    byId.size !== sections.length ||
    sections.some((section) =>
      section.dependencies.some(
        (dependency) =>
          dependency.section_id >= section.id ||
          !byId.has(dependency.section_id),
      ),
    )
  )
    throw new Error("Compass Project Error: Invalid import dependency graph");
  const ordered = [...byId.values()].sort((a, b) => a.id - b.id);
  const positions = new Map(
    ordered.map((section, index) => [section.id, index]),
  );
  const roots = [...explicit].sort((a, b) => a - b);
  const words = Math.ceil(roots.length / 32);
  const required = ordered.map(() => new Uint32Array(words));
  roots.forEach((root, index) => {
    const position = positions.get(root);
    if (position === undefined)
      throw new Error("Compass Project Error: Unknown import section");
    required[position]![index >>> 5]! |= 1 << (index & 31);
  });
  for (let index = ordered.length - 1; index >= 0; index--) {
    const bits = required[index]!;
    if (!bits.some(Boolean)) continue;
    for (const dependency of ordered[index]!.dependencies) {
      const target = required[positions.get(dependency.section_id)!]!;
      for (let word = 0; word < words; word++) target[word]! |= bits[word]!;
    }
  }
  const selection: ImportSelection = {
    included: new Set(),
    requiredBy: new Map(),
  };
  ordered.forEach((section, index) =>
    roots.forEach((root, rootIndex) => {
      if (
        (required[index]![rootIndex >>> 5]! & (1 << (rootIndex & 31))) !==
        0
      ) {
        selection.included.add(section.id);
        if (root !== section.id) {
          let requiredBy = selection.requiredBy.get(section.id);
          if (!requiredBy)
            selection.requiredBy.set(section.id, (requiredBy = new Set()));
          requiredBy.add(root);
        }
      }
    }),
  );
  return selection;
}

export function allSectionIds(preview: ImportPreview): Set<number> {
  return new Set(preview.sections.map((section) => section.id));
}

export function sectionMatches(section: ImportSection, query: string): boolean {
  const normalized = rustTrim(query).toLowerCase();
  return (
    !normalized ||
    section.name.toLowerCase().includes(normalized) ||
    section.relative_path.toLowerCase().includes(normalized)
  );
}

export function toggleSection(
  preview: ImportPreview,
  explicit: Set<number>,
  id: number,
): Set<number> {
  const updated = new Set(explicit);
  if (preview.full_import_reason !== null) return updated;
  let selection: ImportSelection;
  try {
    selection = resolveSelection(preview.sections, explicit);
  } catch {
    return updated;
  }
  if (
    selection.requiredBy.has(id) ||
    !preview.sections.some((section) => section.id === id)
  )
    return updated;
  if (!updated.delete(id)) updated.add(id);
  return updated;
}
