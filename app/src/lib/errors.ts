import type { BackendError } from "./types";

const unitMessages = {
  NoAuthToken: "No auth token set",
  NoUserPreferences: "No user preferences found",
  FilePermissionSet: "Error setting file permissions",
  NoProjectSelected: "No project selected",
  CompassNotFound: "Compass Not Found",
  NoAppHandle: "No app handle available",
} as const;
const prefixes = {
  ProjectAlreadyExists: "Project directory already exists at ",
  ProjectNotFound: "Project not found: ",
  Deserialization: "Error deserializing data: ",
  ProjectFileNotFound: "Referenced file not found: ",
  EmptyProjectDirectory: "Empty project directory for project ID: ",
  NetworkRequest: "Network request error: ",
  Unauthorized: "Unauthorized: ",
  NotFound: "Resource not found: ",
  Unprocessable: "Unprocessable entity: ",
  Conflict: "Conflict: ",
  FileRead: "File read failed: ",
  FileWrite: "File write failed: ",
  NoProjectData: "No project data found for project ",
  ProjectMutexLocked: "Project mutex already locked for project ",
  ZipFile: "Zip File Error: ",
  OsCommand: "Os Command Error: ",
  CompassExecutable: "Compass Executable Error: ",
  CompassProject: "Compass Project Error: ",
  ImportSourceChanged:
    "Source files changed. Refresh the preview before importing: ",
} as const;
const fixedMessages = {
  CreateDirectory: "Couldn't create storage directory for project",
  Serialization: "Error serializing TOML",
  ApiInfoRead: "Error reading user preferece file",
  ApiInfoWrite: "Error writing user preference file",
  ProjectWrite: "Error writing project file",
} as const;
export const UNKNOWN_COMMAND_ERROR =
  "Backend command failed with an unknown error.";

// Keep own-key checks compatible with the supported Safari 15 build target.
function hasOwn(value: object, key: string): boolean {
  return Object.prototype.hasOwnProperty.call(value, key);
}

/** Rust's Display text; permission humanization is applied only to IPC rejections. */
export function formatBackendError(error: BackendError): string {
  if (typeof error === "string") return unitMessages[error];
  if ("Api" in error)
    return `API error ${error.Api.status}: ${error.Api.message}`;
  if ("ProjectImport" in error) {
    const { src_path, dst_path, details } = error.ProjectImport;
    return `Error importing project file from: ${src_path} to ${dst_path}: ${details}`;
  }
  const [key, value] = Object.entries(error)[0];
  if (hasOwn(fixedMessages, key))
    return fixedMessages[key as keyof typeof fixedMessages];
  return `${prefixes[key as keyof typeof prefixes]}${String(value)}`;
}

function isBackendError(value: unknown): value is BackendError {
  if (typeof value === "string") return hasOwn(unitMessages, value);
  if (!value || typeof value !== "object" || Array.isArray(value)) return false;
  const entries = Object.entries(value);
  if (entries.length !== 1) return false;
  const [key, payload] = entries[0];
  if (hasOwn(prefixes, key) || hasOwn(fixedMessages, key))
    return typeof payload === "string";
  if (!payload || typeof payload !== "object") return false;
  if (key === "Api")
    return (
      "status" in payload &&
      typeof payload.status === "number" &&
      Number.isInteger(payload.status) &&
      payload.status >= 0 &&
      payload.status <= 65535 &&
      "message" in payload &&
      typeof payload.message === "string"
    );
  if (key === "ProjectImport")
    return (
      ["src_path", "dst_path", "details"].every(
        (k) => typeof (payload as Record<string, unknown>)[k] === "string",
      ) &&
      "is_permission_error" in payload &&
      typeof payload.is_permission_error === "boolean"
    );
  return false;
}

export class FrontendError extends Error {
  constructor(
    public readonly kind: "ImportSourceChanged" | "Command" | "Serde",
    message: string,
  ) {
    super(message);
    this.name = "FrontendError";
  }
  override toString(): string {
    return this.message;
  }
}

export function normalizeError(value: unknown): FrontendError {
  if (value instanceof FrontendError) return value;
  // The original frontend forwards string rejections literally, including unit variants.
  if (typeof value === "string") return new FrontendError("Command", value);
  if (!isBackendError(value))
    return new FrontendError("Command", UNKNOWN_COMMAND_ERROR);
  if (typeof value !== "string" && "ImportSourceChanged" in value)
    return new FrontendError("ImportSourceChanged", value.ImportSourceChanged);
  if (
    typeof value !== "string" &&
    "ProjectImport" in value &&
    value.ProjectImport.is_permission_error
  ) {
    const source =
      value.ProjectImport.src_path.replace(/\/+$/, "").split("/").pop() ||
      value.ProjectImport.src_path;
    return new FrontendError(
      "Command",
      `Cannot import '${source}' because macOS denied file access. Move/copy the survey folder to an accessible location (for example \`~/Documents\`) and try again.`,
    );
  }
  return new FrontendError("Command", formatBackendError(value));
}
