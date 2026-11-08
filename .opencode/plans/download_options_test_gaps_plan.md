# DownloadOptions & lifecycle test-gap plan

## Context / reflection

The `DownloadOptions` refactor and the deadlock/lifecycle fixes are covered only
*indirectly*: every existing integration test now constructs a `DownloadOptions`
and calls the single `add_download`, so compilation and the basic download paths
are exercised. However, the new API surface and the concurrency fixes are almost
entirely untested. The reviewer caught a deadlock and several lifecycle races
by inspection; nothing in the suite would have caught them or will prevent a
regression.

Confirmed current facts (anchors):

- `DownloadOptions` — `src-tauri/downloader/src/download_options.rs`
  - `new(url, filepath)` defaults: `number_of_segments = 4`,
    `verifier = VerifierMaybe::none()`, `request_headers = RequestHeaders::default()`,
    `retry_scheme = RetryScheme::default()`, `id = None`.
  - Builders: `with_segments(u64)`, `with_id(impl Into<String>)`,
    `with_verifier(VerifierMaybe)`, `with_headers(RequestHeaders)`,
    `with_retry(RetryScheme)`.
- `DownloadManager::add_download(&self, opts: DownloadOptions) -> Result<String, ManagerError>`
  uses `opts.id` when `Some`, otherwise `Uuid::new_v4().to_string()`.
- `DownloadManager::retry(&self, id: &str) -> Result<String, ManagerError>`:
  - only accepts `DownloadState::Stopped | DownloadState::Failed(_)`,
    otherwise `ManagerError::RetryError(state)`;
  - awaits the previous runner via `runner_handles`;
  - re-adds with the same ID.
- `DownloadManager::resume` returns `Ok(())` for an already `Running` item,
  errors with `ResumeError` otherwise (except `Paused`).
- `stop`/`resume`/`retry` serialize on a per-ID `op_locks` mutex and await the
  previous `runner_handles` entry.
- Frontend `DownloadOptionsSchema` is snake_case and mirrors the Rust struct;
  `ManagerErrorSchema` includes `ResumeError` and `RetryError` with a
  `DownloadState` payload.
- Existing tests: `src-tauri/downloader/tests/wiremock_*.rs`,
  `download_*_test.rs`. No test mentions `.retry(`, `with_id`,
  `RetryError`, defaults, or concurrent operations.

## Gaps

1. **No unit tests for `DownloadOptions`**
   - Defaults are never asserted (segments=4, no verifier, default headers,
     default retry, `id=None`).
   - Builder methods are only used incidentally; `with_id`, `with_verifier`,
     `with_headers`, `with_retry` are never asserted to actually replace values,
     and chaining order/overwrite semantics are untested.
   - `impl Into<String>` / `impl Into<PathBuf>` conversions are untested.
   - Serde wire format is untested. A silent field rename (e.g. adding
     `#[serde(rename_all = "camelCase")]`) would break the Tauri command while
     every Rust test still passes.

2. **Explicit vs generated ID is untested**
   - Nothing verifies `add_download(...with_id("fixed"))` returns and stores
     `"fixed"`.
   - Nothing verifies `id: None` produces a non-empty UUID and stores under it.
   - Nothing verifies two `id: None` downloads get distinct IDs.

3. **`retry()` is completely untested**
   - Happy path: stop/fail an item, retry it, verify the same ID is reused,
     a new runner starts, and the file is recreated/overwritten correctly.
   - Guard: `retry()` on `Running`, `Paused`, `Verifying`, `Completed` must
     return `ManagerError::RetryError(state)` and must not replace the item.
   - Guard: `retry()` on an unknown ID returns `IdDoesNotExist`.
   - Regression: no second write to the same path while the old runner is still
     deleting/tearing down (the original race).

4. **`resume()` state machine is untested**
   - New behavior `resume()` on an already `Running` item returns `Ok(())`
     and does not spawn/replace anything.
   - `resume()` on `Completed`/`Stopped`/`Failed` returns `ResumeError(state)`.
   - `resume()` on `Paused` still works (existing live/ignored tests aside).

5. **Runner-handle lifecycle is untested**
   - `stop()` should not return until the runner future has finished and the
     file cleanup happened; a subsequent `retry()`/`resume()` must not have its
     file unlinked by the old runner.
   - `reconstruct()` now populates `runner_handles`; stopping a reconstructed
     item must work.
   - No assertion that `add_download`/`resume`/`reconstruct` leave exactly one
     handle per ID.

6. **Per-ID serialization (`op_locks`) is untested**
   - Concurrent `retry`+`retry`, `stop`+`retry`, `stop`+`resume` on the same ID
     should not spawn two runners or corrupt the file.
   - Without regression tests, the lock could be removed and only a flaky
     failure would reveal it.

7. **Frontend ↔ backend contract is untested**
   - `DownloadOptionsSchema` accepts snake_case fields, but there is no test
     proving the payload that `apiCommunicate.addDownload` sends matches the
     Rust `DownloadOptions` deserializer (`{ opts: { ...snake_case... } }`).
   - `RetryError`/`ResumeError` schema parity is not exercised.

8. **Test hygiene gap that caused the earlier hang**
   - Any new concurrency/lifecycle test must wrap operations in
     `tokio::time::timeout(...)`; a regression to a deadlock must fail the test,
     not hang CI.

---

## Proposed tests

### Step 1 — Unit tests for `DownloadOptions`

**File:** `src-tauri/downloader/src/download_options.rs` (append `#[cfg(test)] mod tests`).

Tests:

- `new_uses_documented_defaults`
  - `let opts = DownloadOptions::new("https://example.com/f.bin", "/tmp/f.bin");`
  - assert `url == "https://example.com/f.bin"`,
    `filepath == PathBuf::from("/tmp/f.bin")`,
    `number_of_segments == 4`,
    `opts.verifier.is_some() == false`,
    `opts.request_headers.user_agent == RequestHeaders::default().user_agent`,
    `opts.retry_scheme.max_attempts == RetryScheme::default().max_attempts`,
    `opts.id.is_none()`.
- `builders_replace_values_and_chain`
  - Build with every builder (`with_segments(1)`, `with_id("abc")`,
    `with_verifier(VerifierMaybe::new(HashAlgo::Sha256, "deadbeef".into()))`,
    `with_headers(custom)`, `with_retry(no_retry_like)`) in one chain and assert
    each field.
- `builder_last_call_wins`
  - `.with_segments(2).with_segments(8)` → 8; same for `with_id`.
- `serde_uses_snake_case_and_matches_frontend_payload`
  - Serialize a fully populated options value; parse as `serde_json::Value`.
  - Assert keys exactly include
    `url`, `filepath`, `number_of_segments`, `verifier`, `request_headers`,
    `retry_scheme`, `id`, and *do not* include camelCase variants.
  - Assert `id` serializes as `null` when `None`.
  - Deserialize a literal JSON string that matches
    `DownloadOptionsSchema` output (snake_case, `id: null`) and assert fields.
- `pathbuf_conversion_accepts_path_and_string`
  - `DownloadOptions::new("u", PathBuf::from("/tmp/a"))` and
    `DownloadOptions::new("u", "/tmp/a")` both give the expected `filepath`.

Note: `VerifierMaybe` only exposes `is_some()` / `algorithm()` /
`expected_hash()`; assert through those accessors.

### Step 2 — Manager tests for IDs and guards

**File:** new `src-tauri/downloader/tests/download_options_manager.rs`
(or extend `wiremock_pause_resume.rs` if preferred).

Reuse `wiremock_helpers::*` (`start_universal_mock_server`,
`default_test_headers`, `default_retry_scheme`, `temp_filepath`).

- `add_download_preserves_explicit_id`
  - Add with `DownloadOptions::new(...).with_segments(1).with_id("chosen-id")`.
  - Assert returned ID == `"chosen-id"`; `manager.download_items.contains_key("chosen-id")`.
- `add_download_generates_uuid_when_id_is_none`
  - Add twice with `id: None`; assert non-empty IDs, different from each other,
    and both keys exist.
- `retry_unknown_id_returns_id_does_not_exist`
  - `manager.retry("missing")` → `Err(ManagerError::IdDoesNotExist)`.
- `retry_rejects_non_terminal_states`
  - Build/add a download against a slow wiremock server; while state is
    `Running` (poll with timeout) call `retry` and assert
    `RetryError(DownloadState::Running { .. })`.
  - For `Completed`: use a tiny successful server, wait for completion, assert
    `RetryError(DownloadState::Completed { .. })`.
  - (Optional) same for `Paused`.
- `resume_running_is_noop`
  - Add/start a download; wait until `Running`; call `resume` twice; assert
    both `Ok(())` and the item Arc is the same/only one runner handle for the ID.
- `resume_rejects_completed`
  - Wait for `Completed`; `resume` → `ResumeError(DownloadState::Completed { .. })`.

### Step 3 — Retry happy path and old-runner teardown

**File:** `src-tauri/downloader/tests/wiremock_retry.rs`.

- `retry_stopped_download_reuses_id_and_restarts`
  - `add_download` with 1 segment against a slow server.
  - `stop(&id).await` (with timeout).
  - `retry(&id).await` → `Ok(id)`.
  - Assert `manager.download_items.contains_key(&id)` and only one entry whose
    ID is `id`.
  - Wait for `Completed`; assert file content equals expected and `downloaded`
    equals the server size.
  - This directly guards the original stop-cleanup race.
- `retry_after_failure_recreates_file`
  - Configure a mock that fails the first request (e.g. 500 or reset) with
    `no_retry_scheme` so the item reaches `Failed`.
  - `retry` with a mock now returning success; wait for `Completed`; verify file.
- `retry_does_not_delete_new_file`
  - Start download; `stop().await` (which now awaits the runner); immediately
    `retry`; then assert the file still exists and eventually matches content.
  - Wrap the whole test in `tokio::time::timeout(Duration::from_secs(15), ...)`
    so a resumed deadlock fails instead of hanging.

### Step 4 — Concurrency / per-ID lock regressions

**File:** `src-tauri/downloader/tests/wiremock_retry_concurrency.rs`.

All tests body wrapped in `tokio::time::timeout` (5–15 s).

- `concurrent_retry_calls_spawn_one_runner`
  - Arrange a `Stopped` item.
  - `tokio::join!(manager.retry(&id), manager.retry(&id))`.
  - Expect exactly one success and one `RetryError`/`IdDoesNotExist` (document
    which is intended; with the current implementation the loser should see
    `RetryError(Running)` because the first has already replaced the item).
  - Assert `manager.runner_handles.len()` for that ID is 1 and the download
    completes once.
- `stop_then_concurrent_retry_keeps_file`
  - Spawn `stop` and `retry` concurrently with `tokio::join!`.
  - Allow either ordering; final invariant: no panic/hang, and after completion
    the file exists with correct bytes (or the retry returned `RetryError`).
- `stop_and_resume_do_not_deadlock`
  - `tokio::join!(manager.stop(&id), manager.resume(&id))` with timeout.
  - Assert no deadlock and that the item ends in a consistent terminal state.

### Step 5 — Reconstructed runner-handle lifecycle

**File:** extend `src-tauri/downloader/tests/wiremock_pause_resume.rs`.

- `stop_after_reconstruct_completes`
  - Reconstruct a running item, call `stop` with timeout, assert state
    `Stopped`, file removed, and `runner_handles` no longer contains the ID.
- `resume_after_reconstruct_keeps_single_runner`
  - Existing `test_reconstruct_resume` already covers resume; add an assertion
    that `new_manager.runner_handles.len() == 1`.

### Step 6 — Frontend contract test (lightweight)

No frontend test runner is currently configured, so avoid adding Vitest just
for this unless desired. Two pragmatic options:

- **Rust-side contract test (recommended, no new tooling):**
  In `download_options.rs` tests, keep the literal frontend-shaped JSON
  (`snake_case`, `"id": null`) from Step 1. This catches Rust-side drift.
- **Optional TS test (requires Vitest):**
  Add `src/code/typesOfBackend.test.ts`:
  - `DownloadOptionsSchema.safeParse(frontendShapedObject).success === true`
  - parsing rejects camelCase `numberOfSegments` (documents the wire contract)
  - `ManagerApiErrorSchema` parses
    `{ message: "x", kind: "RetryError", payload: { kind: "Completed", payload: { size: 1 } } }`
    and the `ResumeError` equivalent.
  - Also assert `addDownload` calls `invoke("add_download", { opts: parsed })`
    using a mocked `@tauri-apps/api/core`.
  - Only add this if a TS test runner is acceptable; otherwise rely on
    `tsc --noEmit` plus the Rust contract test.

## Verification commands

From `src-tauri/downloader`:

```bash
timeout 300 cargo test --lib
timeout 300 cargo test --test download_options_manager
timeout 300 cargo test --test wiremock_retry
timeout 300 cargo test --test wiremock_retry_concurrency
timeout 300 cargo test --test wiremock_pause_resume
timeout 300 cargo test            # full suite
```

From repo root:

```bash
timeout 180 npx tsc --noEmit
```

If Vitest is added:

```bash
timeout 180 npm test
```

## Edge cases to encode explicitly

- `id: Some("")` — currently accepted; decide/assert behavior (likely preserved
  as an empty key; if undesirable, reject with a `ManagerError`).
- URL/path with non-UTF8 path — `PathBuf` serde behavior; not required now.
- `number_of_segments = 0` — currently reaches `get_download_data`; add a test
  documenting current behavior or a validation error.
- `retry()` while the old runner is in `Verifying` (file hash read) — must be
  rejected or must await completion; test whichever contract is chosen.
- Concurrent `retry` with different options? `retry()` only derives options from
  the existing item; ensure headers/verifier/retry scheme are preserved.
- Timeout every async wait (`wait_for_state` loops already have limits; keep
  using `tokio::time::timeout` around new `retry`/`stop`/`resume` calls).
