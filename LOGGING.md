# Application logs

Native logs are written under `~/.compass/` with the filename pattern
`speleodb_compass*.log`. The startup record includes the running application
version. HTTP, synchronization, project operations and operating-system errors
remain logged by Rust after the frontend transition.

## View logs

```sh
tail -f ~/.compass/speleodb_compass*.log
rg "Downloading project|Download failed|404" ~/.compass/speleodb_compass*.log
```

For frontend problems during development, inspect the WebView console alongside
native logs. Distinguish an IPC invocation failure from a native HTTP error; API
status mapping and server-provided messages remain owned by `api/src/http.rs`.
Do not include OAuth tokens or private project contents in shared diagnostic
reports.

## Download diagnostics

A download records its project ID before sending the request:

```text
Downloading project zip for project: {project_id}
Download request completed for project {project_id} in ...
```

Failures record `Download failed for project {project_id}: ...`. The endpoint is
the configured instance followed by
`api/v2/projects/{project_id}/download/compass_zip/`; instances hosted below a
path prefix retain that prefix. Project-info fetch logs also show the requested
URL. Correlate the project ID, request timing and server-provided error message
when investigating a failed download.

## Debugging a 404

Check the configured instance and any path prefix, the project ID, and whether
the server provides the v2 endpoint above. A 404 maps to `NotFound` with the
server's message; the UI displays that typed failure. A 422 download response
maps separately to `NoProjectData`, so an empty project should not be diagnosed
as a missing route. These HTTP mappings remain centralized in `api/src/http.rs`
and `api/src/project.rs`.
