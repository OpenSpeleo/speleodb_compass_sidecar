/** The existing Rust/Serde wire contract. Keep casing and nullability intact. */
export type BackendError =
  | "NoAuthToken"
  | "NoUserPreferences"
  | "FilePermissionSet"
  | "NoProjectSelected"
  | "CompassNotFound"
  | "NoAppHandle"
  | { ProjectAlreadyExists: string }
  | { ProjectNotFound: string }
  | { CreateDirectory: string }
  | { Deserialization: string }
  | { Serialization: string }
  | { ApiInfoRead: string }
  | { ApiInfoWrite: string }
  | {
      ProjectImport: {
        src_path: string;
        dst_path: string;
        details: string;
        is_permission_error: boolean;
      };
    }
  | { ProjectWrite: string }
  | { ProjectFileNotFound: string }
  | { EmptyProjectDirectory: string }
  | { NetworkRequest: string }
  | { Unauthorized: string }
  | { NotFound: string }
  | { Unprocessable: string }
  | { Conflict: string }
  | { Api: { status: number; message: string } }
  | { FileRead: string }
  | { FileWrite: string }
  | { NoProjectData: string }
  | { ProjectMutexLocked: string }
  | { ZipFile: string }
  | { OsCommand: string }
  | { CompassExecutable: string }
  | { CompassProject: string }
  | { ImportSourceChanged: string };

export interface ActiveMutex {
  user: string;
  creation_date: string;
  modified_date: string;
}
export interface CommitInfo {
  id: string;
  message: string;
  author_name: string;
  commit_date?: string;
  dt_since: string;
}
export type ProjectType = "COMPASS" | "IGNORED";
export interface ProjectInfo {
  id: string;
  name: string;
  description: string;
  is_active: boolean;
  permission: string;
  active_mutex: ActiveMutex | null;
  country: string;
  created_by: string;
  creation_date: string;
  modified_date: string;
  latitude?: number;
  longitude?: number;
  fork_from: string | null;
  visibility: string;
  exclude_geojson: boolean;
  latest_commit: CommitInfo | null;
  type: ProjectType;
}
export type ProjectSaveResult = "Saved" | "NoChanges";
export type LocalProjectStatus =
  | "Unknown"
  | "RemoteOnly"
  | "EmptyLocal"
  | "Dirty"
  | "UpToDate"
  | "OutOfDate"
  | "DirtyAndOutOfDate";
export interface ProjectStatus {
  local_status: LocalProjectStatus;
  info: ProjectInfo;
}
export type LoadingState =
  | "NotStarted"
  | "LoadingPrefs"
  | "Authenticating"
  | "LoadingProjects"
  | "Unauthenticated"
  | "Ready"
  | { Failed: BackendError };
export type Platform = "Windows" | "MacOS" | "Linux";
export type UpdateNotificationPhase =
  | "Checking"
  | { Downloading: { version: string; progress_percent: number | null } }
  | { Installing: { version: string } }
  | { Relaunching: { version: string } }
  | { UpToDate: { app_name: string } }
  | { Failed: { message: string } };
export interface UpdateNotification {
  id: number;
  phase: UpdateNotificationPhase;
}
export interface UiState {
  loading_state: LoadingState;
  platform: Platform;
  user_email: string | null;
  project_status: ProjectStatus[];
  selected_project_id: string | null;
  compass_open: boolean;
  project_downloading: boolean;
  update_notification: UpdateNotification | null;
}
export interface ImportDependency {
  section_id: number;
  reason: string;
}
export interface ImportSection {
  id: number;
  name: string;
  relative_path: string;
  dependencies: ImportDependency[];
}
export interface ImportPreview {
  preview_id: string;
  source_name: string;
  source_directory: string;
  sections: ImportSection[];
  full_import_reason: string | null;
}
export type InitialImportOutcome =
  | { Synced: { save_result: ProjectSaveResult } }
  | { LocalOnly: { error: BackendError } }
  | { UploadedNeedsRefresh: { error: BackendError } };
export interface AboutInfo {
  version: string;
  repo: string;
  authors: string[];
  description: string;
}
