import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { controller } from "../lib/controller";
import { FrontendError } from "../lib/errors";
import {
  allSectionIds,
  resolveSelection,
  sectionMatches,
  toggleSection,
  type ImportSelection,
} from "../lib/import-selection";
import type { ImportPreview, InitialImportOutcome } from "../lib/types";

type ImportPhase =
  "Picking" | "Analyzing" | "Reviewing" | "Importing" | "Failed";
interface ImportFlow {
  phase: ImportPhase;
  sourcePath: string | null;
  preview: ImportPreview | null;
  error: string | null;
  needsRefresh: boolean;
}
const initialFlow = (): ImportFlow => ({
  phase: "Picking",
  sourcePath: null,
  preview: null,
  error: null,
  needsRefresh: false,
});

/** Synchronous guards also cover repeated clicks before React renders. */
export class ImportRequests {
  generation = 0;
  mounted = true;
  reading = false;
  submitting = false;
  previewId: string | null = null;
  begin() {
    return ++this.generation;
  }
  accepts(generation: number) {
    return this.mounted && this.generation === generation;
  }
  startImport() {
    if (!this.mounted || this.reading || this.submitting) return null;
    this.submitting = true;
    return this.begin();
  }
  startRead() {
    if (!this.mounted || this.submitting || this.reading) return null;
    this.reading = true;
    return this.begin();
  }
  takePreview() {
    const id = this.previewId;
    this.previewId = null;
    return id;
  }
}
function releasePreview(previewId: string | null) {
  if (previewId !== null)
    void controller
      .cancelCompassImport(previewId)
      .catch((error) =>
        console.warn("Could not release import preview:", error),
      );
}
const messageOf = (error: unknown): string =>
  error instanceof Error ? error.message : String(error);

async function analyzeSource(
  projectId: string,
  sourcePath: string,
  setFlow: (flow: ImportFlow) => void,
  requests: ImportRequests,
  generation: number,
) {
  releasePreview(requests.takePreview());
  setFlow({ ...initialFlow(), phase: "Analyzing", sourcePath });
  try {
    const preview = await controller.previewCompassImport(
      projectId,
      sourcePath,
    );
    if (!requests.accepts(generation)) {
      releasePreview(preview.preview_id);
      return;
    }
    requests.reading = false;
    requests.previewId = preview.preview_id;
    setFlow({
      phase: "Reviewing",
      sourcePath,
      preview,
      error: null,
      needsRefresh: false,
    });
  } catch (error) {
    if (!requests.accepts(generation)) return;
    requests.reading = false;
    setFlow({
      ...initialFlow(),
      phase: "Failed",
      sourcePath,
      error: messageOf(error),
    });
  }
}
async function chooseSource(
  projectId: string,
  previous: ImportFlow,
  setFlow: (flow: ImportFlow) => void,
  requests: ImportRequests,
  onClose: () => void,
) {
  const generation = requests.startRead();
  if (generation === null) return;
  setFlow({ ...previous, phase: "Picking" });
  try {
    const sourcePath = await controller.pickCompassProjectFile();
    if (!requests.accepts(generation)) return;
    if (sourcePath !== null) {
      await analyzeSource(projectId, sourcePath, setFlow, requests, generation);
    } else {
      requests.reading = false;
      if (previous.sourcePath !== null) setFlow(previous);
      else onClose();
    }
  } catch (error) {
    if (!requests.accepts(generation)) return;
    requests.reading = false;
    setFlow({
      ...previous,
      phase: previous.preview ? "Reviewing" : "Failed",
      error: messageOf(error),
    });
  }
}

export interface InitialImportModalProps {
  projectId: string;
  onClose: () => void;
  onComplete: (outcome: InitialImportOutcome, count: number) => void;
}
export function InitialImportModal({
  projectId,
  onClose,
  onComplete,
}: InitialImportModalProps) {
  const [flow, setFlow] = useState(initialFlow);
  const requestsRef = useRef(new ImportRequests());
  const onCloseRef = useRef(onClose);
  onCloseRef.current = onClose;
  useEffect(() => {
    const requests = new ImportRequests();
    requestsRef.current = requests;
    // Defer the native picker until after development effect replay cleanup.
    queueMicrotask(() => {
      if (requests.mounted)
        void chooseSource(projectId, initialFlow(), setFlow, requests, () =>
          onCloseRef.current(),
        );
    });
    return () => {
      requests.mounted = false;
      requests.begin();
      if (!requests.submitting) releasePreview(requests.takePreview());
    };
  }, [projectId]);
  function cancel() {
    const requests = requestsRef.current;
    if (requests.submitting) return;
    requests.begin();
    releasePreview(requests.takePreview());
    onClose();
  }
  function choose() {
    void chooseSource(projectId, flow, setFlow, requestsRef.current, onClose);
  }
  function refresh() {
    if (flow.sourcePath === null) return;
    const requests = requestsRef.current;
    const generation = requests.startRead();
    if (generation !== null)
      void analyzeSource(
        projectId,
        flow.sourcePath,
        setFlow,
        requests,
        generation,
      );
  }
  async function confirm(explicit: number[]) {
    const requests = requestsRef.current;
    if (
      requests.submitting ||
      flow.phase !== "Reviewing" ||
      flow.needsRefresh ||
      !explicit.length ||
      !flow.preview
    )
      return;
    const preview = flow.preview;
    if (requests.previewId !== preview.preview_id) return;
    let selection: ImportSelection;
    try {
      selection = resolveSelection(preview.sections, new Set(explicit));
    } catch {
      return;
    }
    const generation = requests.startImport();
    if (generation === null) return;
    const previous = flow;
    setFlow({ ...previous, phase: "Importing", error: null });
    try {
      const outcome = await controller.confirmCompassImport(
        preview.preview_id,
        explicit,
      );
      requests.submitting = false;
      if (!requests.accepts(generation)) {
        releasePreview(requests.takePreview());
        return;
      }
      requests.previewId = null;
      onComplete(outcome, selection.included.size);
    } catch (error) {
      requests.submitting = false;
      if (!requests.accepts(generation)) {
        releasePreview(requests.takePreview());
        return;
      }
      setFlow({
        ...previous,
        phase: "Reviewing",
        needsRefresh:
          error instanceof FrontendError &&
          error.kind === "ImportSourceChanged",
        error: messageOf(error),
      });
    }
  }
  if (
    flow.phase === "Picking" &&
    flow.sourcePath === null &&
    flow.error === null
  )
    return null;
  const importing = flow.phase === "Importing";
  const working = flow.phase === "Picking" || flow.phase === "Analyzing";
  return (
    <ImportDialog onCancel={cancel} busy={importing}>
      {flow.preview ? (
        <SectionSelector
          key={flow.preview.preview_id}
          preview={flow.preview}
          onConfirm={(ids) => void confirm(ids)}
          onCancel={cancel}
          onChooseFile={choose}
          onRefresh={refresh}
          busy={importing || working}
          needsRefresh={flow.needsRefresh}
          error={flow.error}
        />
      ) : (
        <>
          <header className="import-selector__header">
            <div className="import-selector__eyebrow">COMPASS IMPORT</div>
            <h2 id="import-selector-title" tabIndex={-1}>
              Choose sections to import
            </h2>
            <p id="import-selector-description">
              Bring the parts of your survey you need into this project.
            </p>
          </header>
          <div
            className="import-selector__loading"
            aria-live="polite"
            aria-busy={working}
          >
            {working ? (
              <>
                <span className="import-selector__spinner" aria-hidden="true" />
                <h3>Reading your project</h3>
                <p>Checking sections and their connections…</p>
              </>
            ) : (
              <>
                <h3>This project needs a closer look</h3>
                <p role="alert">{flow.error ?? ""}</p>
              </>
            )}
          </div>
          <footer className="import-selector__footer">
            <span className="import-selector__footnote">
              Your original files stay untouched.
            </span>
            <div className="import-selector__actions">
              <button
                type="button"
                className="import-selector__secondary"
                onClick={cancel}
              >
                Cancel
              </button>
              {!working && (
                <>
                  <button
                    type="button"
                    className="import-selector__secondary"
                    onClick={choose}
                  >
                    Choose another file
                  </button>
                  {flow.sourcePath !== null && (
                    <button
                      type="button"
                      className="import-selector__primary"
                      onClick={refresh}
                    >
                      Try again
                    </button>
                  )}
                </>
              )}
            </div>
          </footer>
        </>
      )}
    </ImportDialog>
  );
}

export function ImportDialog({
  children,
  onCancel,
  busy = false,
}: {
  children: ReactNode;
  onCancel: () => void;
  busy?: boolean;
}) {
  const dialogRef = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    const dialog = dialogRef.current;
    if (!dialog) return;
    try {
      dialog.showModal();
    } catch (error) {
      console.error("Could not open import dialog:", error);
    }
    dialog.querySelector<HTMLElement>("#import-selector-title")?.focus();
    return () => {
      dialog.close();
    };
  }, []);
  return (
    <dialog
      ref={dialogRef}
      className="import-selector"
      aria-labelledby="import-selector-title"
      aria-describedby="import-selector-description"
      aria-modal="true"
      onCancel={(event) => {
        event.preventDefault();
        if (!busy) onCancel();
      }}
    >
      {children}
    </dialog>
  );
}

export interface SectionSelectorProps {
  preview: ImportPreview;
  onConfirm: (ids: number[]) => void;
  onCancel: () => void;
  onChooseFile: () => void;
  onRefresh: () => void;
  busy?: boolean;
  needsRefresh?: boolean;
  error?: string | null;
}
export function SectionSelector({
  preview,
  onConfirm,
  onCancel,
  onChooseFile,
  onRefresh,
  busy = false,
  needsRefresh = false,
  error: suppliedError = null,
}: SectionSelectorProps) {
  const [explicit, setExplicit] = useState(() => allSectionIds(preview));
  const [query, setQuery] = useState("");
  const [expanded, setExpanded] = useState(new Set<number>());
  const headingRef = useRef<HTMLHeadingElement>(null);
  useEffect(() => {
    headingRef.current?.focus();
  }, []);
  const { selection, graphError } = useMemo(() => {
    try {
      return {
        selection: resolveSelection(preview.sections, explicit),
        graphError: null,
      };
    } catch (error) {
      return {
        selection: {
          included: new Set<number>(),
          requiredBy: new Map<number, Set<number>>(),
        },
        graphError: messageOf(error),
      };
    }
  }, [preview, explicit]);
  const byId = useMemo(
    () => new Map(preview.sections.map((section) => [section.id, section])),
    [preview],
  );
  const connectionReasons = useMemo(() => {
    const reasons = new Map<number, Array<[number, string]>>();
    for (const candidate of preview.sections)
      for (const dependency of candidate.dependencies) {
        const values = reasons.get(dependency.section_id) ?? [];
        values.push([candidate.id, `${candidate.name}: ${dependency.reason}`]);
        reasons.set(dependency.section_id, values);
      }
    return reasons;
  }, [preview]);
  const total = preview.sections.length;
  const count = selection.included.size;
  const automatic = Math.max(0, count - explicit.size);
  const fullOnly = preview.full_import_reason !== null;
  const disabled = busy || needsRefresh;
  const allSelected = explicit.size === total;
  const matches = preview.sections.filter((section) =>
    sectionMatches(section, query),
  );
  const error = suppliedError ?? graphError;
  return (
    <>
      <header className="import-selector__header">
        <div className="import-selector__eyebrow">COMPASS IMPORT</div>
        <h2 ref={headingRef} id="import-selector-title" tabIndex={-1}>
          Choose sections to import
        </h2>
        <p id="import-selector-description">
          Choose the sections you need. Required connections are included
          automatically.
        </p>
        <div className="import-selector__source">
          <span className="import-selector__file-icon" aria-hidden="true">
            ↳
          </span>
          <div>
            <strong>{preview.source_name}</strong>
            <span>{preview.source_directory}</span>
          </div>
          <button
            type="button"
            className="import-selector__text-button"
            disabled={busy}
            onClick={onChooseFile}
          >
            Change file
          </button>
        </div>
      </header>
      {preview.full_import_reason !== null && (
        <div className="import-selector__notice" role="status">
          <strong>This project needs to stay together</strong>
          <p>{preview.full_import_reason}</p>
          <p>You can import the complete project. All sections are included.</p>
        </div>
      )}
      {error !== null && (
        <div className="import-selector__error" role="alert">
          <strong>
            {needsRefresh
              ? "The source files changed"
              : "Import could not finish"}
          </strong>
          <p>{error}</p>
          {needsRefresh && (
            <p>
              Refresh to review the updated project. All sections will be
              selected again.
            </p>
          )}
          <button
            type="button"
            className="import-selector__text-button"
            disabled={busy}
            onClick={onRefresh}
          >
            Refresh preview
          </button>
        </div>
      )}
      <div className="import-selector__toolbar">
        <label className="import-selector__search">
          <span className="import-selector__sr-only">Search sections</span>
          <svg
            aria-hidden="true"
            width="17"
            height="17"
            viewBox="0 0 20 20"
            fill="none"
          >
            <circle
              cx="8.5"
              cy="8.5"
              r="5.5"
              stroke="currentColor"
              strokeWidth="1.7"
            />
            <path
              d="m13 13 4 4"
              stroke="currentColor"
              strokeWidth="1.7"
              strokeLinecap="round"
            />
          </svg>
          <input
            type="search"
            placeholder="Find a section…"
            value={query}
            onChange={(event) => setQuery(event.currentTarget.value)}
            disabled={busy}
          />
        </label>
        {!fullOnly && (
          <div className="import-selector__bulk">
            <button
              type="button"
              className="import-selector__text-button"
              disabled={disabled || allSelected}
              onClick={() => setExplicit(allSectionIds(preview))}
            >
              Select all
            </button>
            <span aria-hidden="true">·</span>
            <button
              type="button"
              className="import-selector__text-button"
              disabled={disabled || explicit.size === 0}
              onClick={() => setExplicit(new Set())}
            >
              Clear selection
            </button>
          </div>
        )}
      </div>
      <div
        className="import-selector__list"
        aria-label="Project sections"
        aria-busy={busy}
      >
        {!matches.length && (
          <div className="import-selector__no-results">
            <strong>No matching sections</strong>
            <p>Try a different name or clear your search.</p>
            <button
              type="button"
              className="import-selector__text-button"
              onClick={() => setQuery("")}
            >
              Clear search
            </button>
          </div>
        )}
        {matches.map((section) => {
          const checked = selection.included.has(section.id);
          const required = selection.requiredBy.get(section.id);
          const locked = required !== undefined;
          const roots = required ? [...required] : [];
          const names = roots
            .slice(0, 2)
            .flatMap((id) => byId.get(id)?.name ?? []);
          const explanationId = `import-section-reason-${section.id}`;
          const checkboxId = `import-section-${section.id}`;
          const requiredText =
            roots.length > 2
              ? `Required by ${names[0]} and ${roots.length - 1} others`
              : `Required by ${names.join(" and ")}`;
          return (
            <div
              key={section.id}
              className={`import-selector__row${checked ? " import-selector__row--selected" : ""}`}
            >
              <input
                id={checkboxId}
                type="checkbox"
                checked={checked}
                disabled={disabled || locked || fullOnly}
                aria-describedby={locked ? explanationId : undefined}
                onChange={() => {
                  if (!disabled)
                    setExplicit((previous) =>
                      toggleSection(preview, previous, section.id),
                    );
                }}
              />
              <div className="import-selector__row-content">
                <label htmlFor={checkboxId}>
                  <strong>{section.name}</strong>
                  <span className="import-selector__path">
                    {section.relative_path}
                  </span>
                </label>
                {locked && (
                  <details
                    className="import-selector__dependency"
                    open={expanded.has(section.id)}
                    onToggle={(event) => {
                      const open = event.currentTarget.open;
                      setExpanded((previous) => {
                        if (previous.has(section.id) === open) return previous;
                        const next = new Set(previous);
                        if (open) next.add(section.id);
                        else next.delete(section.id);
                        return next;
                      });
                    }}
                  >
                    <summary id={explanationId}>{requiredText}</summary>
                    {expanded.has(section.id) && (
                      <ul>
                        {roots
                          .map((id) => byId.get(id))
                          .filter((root) => root !== undefined)
                          .map((root) => (
                            <li key={`root-${root.id}`}>
                              {`Required by ${root.name}`}
                            </li>
                          ))}
                        {(connectionReasons.get(section.id) ?? [])
                          .filter(([id]) => selection.included.has(id))
                          .map(([id, reason], index) => (
                            <li key={`reason-${id}-${index}`}>{reason}</li>
                          ))}
                      </ul>
                    )}
                  </details>
                )}
              </div>
              {locked && (
                <span className="import-selector__badge">Required</span>
              )}
            </div>
          );
        })}
      </div>
      <div
        className="import-selector__summary"
        role="status"
        aria-live="polite"
        aria-atomic="true"
      >
        <strong>{`${count} of ${total} sections`}</strong>
        <span>
          {automatic > 0
            ? `${automatic} included automatically`
            : count === 0
              ? "Select at least one section to continue."
              : allSelected && !fullOnly
                ? "Import everything, or clear the selection to choose a smaller set."
                : "Your original files stay untouched."}
        </span>
      </div>
      <footer className="import-selector__footer">
        {busy ? (
          <div className="import-selector__progress" role="status">
            <span
              className="import-selector__spinner import-selector__spinner--small"
              aria-hidden="true"
            />
            <span>Preparing your selection and syncing with SpeleoDB…</span>
          </div>
        ) : (
          <span className="import-selector__footnote">
            A new copy. Just your selection.
          </span>
        )}
        <div className="import-selector__actions">
          <button
            type="button"
            className="import-selector__secondary"
            disabled={busy}
            onClick={onCancel}
          >
            Cancel
          </button>
          <button
            type="button"
            className="import-selector__primary"
            disabled={disabled || count === 0}
            onClick={() => {
              if (!disabled && explicit.size > 0 && graphError === null)
                onConfirm([...explicit].sort((a, b) => a - b));
            }}
          >
            {busy
              ? "Importing…"
              : fullOnly
                ? "Import complete project"
                : `Import ${count} ${count === 1 ? "section" : "sections"}`}
          </button>
        </div>
      </footer>
    </>
  );
}
