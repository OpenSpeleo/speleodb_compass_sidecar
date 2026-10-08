// @vitest-environment node
import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { expect, it } from "vitest";

const bunVersion = readFileSync(
  new URL("../.bun-version", import.meta.url),
  "utf8",
).trim();

it("runs the Vitest worker on the pinned Bun runtime", () => {
  expect(process.versions.bun).toBe(bunVersion);
});

it("runs tools with a node shebang through the inherited Bun shim", () => {
  const child = spawnSync("node", ["-p", "process.versions.bun"], {
    encoding: "utf8",
  });
  expect(child.error).toBeUndefined();
  expect(child.status).toBe(0);
  expect(child.stdout.trim()).toBe(bunVersion);
});
