import { useState } from "react";
import { createRoot } from "react-dom/client";
import {
  ImportDialog,
  SectionSelector,
} from "../src/components/import-selector";
import type { ImportPreview } from "../src/lib/types";
import "../styles.css";

const params = new URLSearchParams(location.search);
const count = Number(params.get("dense") ?? 20);
const busy = params.has("busy");
const preview: ImportPreview = {
  preview_id: "browser-fixture",
  source_name: "Mammoth Cave.mak",
  source_directory: "Surveys / Kentucky / Mammoth Cave",
  full_import_reason: params.has("warning")
    ? `Connections in ${"directory/".repeat(30)}Survey.DAT could not be verified. Import all sections.`
    : null,
  sections: Array.from({ length: count }, (_, id) => ({
    id,
    name: `Passage ${String(id).padStart(3, "0")}.DAT`,
    relative_path: `North Entrance / Passage ${String(id).padStart(3, "0")}.DAT`,
    dependencies:
      id > 0
        ? [{ section_id: id - 1, reason: `Connects at station A${id}` }]
        : [],
  })),
};
function Fixture() {
  const [open, setOpen] = useState(true);
  const [confirmed, setConfirmed] = useState<number[] | null>(null);
  return (
    <>
      {!open && <p role="status">Dialog closed</p>}
      {confirmed !== null && (
        <output data-testid="confirmed">{JSON.stringify(confirmed)}</output>
      )}
      {open && (
        <ImportDialog onCancel={() => setOpen(false)} busy={busy}>
          <SectionSelector
            preview={preview}
            onConfirm={setConfirmed}
            onCancel={() => setOpen(false)}
            onChooseFile={() => {}}
            onRefresh={() => {}}
            busy={busy}
            needsRefresh={params.has("stale")}
            error={
              params.has("error")
                ? "The server could not complete this request. ".repeat(50)
                : null
            }
          />
        </ImportDialog>
      )}
    </>
  );
}
createRoot(document.getElementById("root")!).render(<Fixture />);
