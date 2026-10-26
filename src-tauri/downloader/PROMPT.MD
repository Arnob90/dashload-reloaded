Inspect the `dashload-downloader` crate to prepare a documentation plan:

1. Read `tests/` first 
   Use these tests as the authoritative blueprint for:
   - How `DownloadManager` is initialized.
   - How `add_download` is called with `VerifierMaybe` and `RequestHeaders`.
   - How status is polled and matched against `DownloadState::Completed` and `DownloadState::Running`.
   - How pause and resume lifecycles work.

2. Inspect `src/lib.rs` to identify all public (`pub`) exports and structs.

3. Output `.opencode/plans/docs_plan.md` outlining:
   - The primary user-facing workflow.
   - The core public types (`DownloadManager`, `DownloadStatus`, `VerifierMaybe`, `RequestHeaders`).
   - 2 accurate, compiling code examples derived directly from the working tests.
This will be used by another agent to create the documentation for this project. Keep this in mind.
