# Plan: Refactor `InPlaceSegment::start_internal` into a State Machine

## Objective

Add a new, fully isolated `start_internal_refactored` implementation to
`src-tauri/downloader/src/in_place_segment.rs`, plus the supporting
`handle_connect`, `handle_stream`, and `handle_backoff` handlers.

**Do not replace or modify the existing `start_internal`.** The refactored
driver and handlers will be dead code until the user renames
`start_internal_refactored` to `start_internal`. This preserves the current
working path and makes the change reviewable in one file.

---

## 1. Current code facts (anchors)

All code is in `src-tauri/downloader/src/in_place_segment.rs`.

| Item | Current location / signature |
| --- | --- |
| `DownloadingState` | Lines 55-61, currently unit-only `Connect`, `Download`, `Retry`, `Finish` |
| `start()` | Lines 163-176, calls `self.start_internal()` and publishes `Failed` on error |
| `get_next_chunk_or_cancel()` | Lines 146-159, clones the watch receiver and selects on `res.chunk()` vs `receiver.changed()` |
| `stream_chunks()` | Lines 179-239, writes chunks, updates `downloaded`, emits `Running` progress, returns `StreamChunkResult::{Finished, Interrupted}` |
| `fetch_res_unless_interrupt()` | Lines 240-280, currently used only by the half-finished `connect` |
| `connect()` | Lines 281-337, half-finished, returns `DownloadingState`, contains `todo!()`, and ignores `downloaded` when building the range header |
| `start_internal()` | Lines 341-539, the legacy implementation to preserve |
| `retry_internal()` | Lines 549-566, emits `DownloadState::Retrying` and returns the backoff delay; expects `attempts >= 1` |
| `get_range_header()` | Lines 570-579, currently reads `data.last_downloaded` instead of the live `downloaded` value |
| `calculate_speed()` | Lines 582-585 |

Relevant public/crate types:

```rust
pub enum SegmentCommand { Stop, Pause, Nothing }

pub enum StreamChunkResult {
    Finished,
    Interrupted(SegmentCommand),
}

pub enum NextChunkMaybe {
    Interrupted(SegmentCommand),
    Chunk(Option<Bytes>),
}

pub struct RetryScheme {
    pub max_attempts: u32,
    pub backoff: ExponentialBackoff,
}
```

Important existing behavior to preserve:

1. `start_internal` exits immediately with `Ok(())` when the segment status is
   not `DownloadState::Running`.
2. `total_size` is `info.size()` for segmented downloads, otherwise
   `get_total_size(...)`.
3. If `total_size` is known and `last_downloaded >= size`, publish
   `DownloadState::Completed { size }` and return without opening the file.
4. The file is opened with `write(true).create(true).truncate(false)`.
5. Before connecting, the file is sought to `info.start.unwrap_or(0) + downloaded`.
6. A segmented request sends `Range: bytes={start + downloaded}-{end}`.
   A non-segmented request sends `bytes={downloaded}-` only when
   `downloaded > 0`.
7. `SegmentCommand::Pause` publishes `Paused`, flushes the file, and exits.
   `SegmentCommand::Stop` publishes `Stopped` and exits. `Stop` must return
   `Ok(())` so `DownloadItem::start` can delete the file.
8. A successful handshake resets `attempts = 0`.
9. Handshake errors and `stream_chunks` errors of type
   `DownloadRunError::NetworkError(_)` or `DownloadRunError::TimeoutError` are
   retryable while `attempts < self.retry_scheme.max_attempts`.
10. `retry_internal(attempts, ...)` is called with the incremented attempt
    count, emits `Retrying`, and its returned delay is slept before reconnecting.
11. On success, the final status is
    `Completed { size: downloaded }` with `total: Some(downloaded)`.
12. A `Range` request that does not receive `206 Partial Content` is a
    `ServerError`; other non-2xx responses are a `NetworkError`.

---

## 2. Target state machine design

### 2.1 `DownloadingState`

Keep the enum `Copy` as specified in the prompt. `File` and `Response` are
non-`Copy` resources and therefore live in a private session context instead of
inside the enum.

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DownloadingState {
    Connect { downloaded: u64, attempts: u32 },
    Download { downloaded: u64, attempts: u32 },
    Retry { downloaded: u64, attempts: u32 },
    Finish { downloaded: u64, completed: bool },
}
```

`Finish.completed` distinguishes:

- `true`: the stream reached EOF normally; the driver publishes `Completed`.
- `false`: pause or stop already published `Paused`/`Stopped`; the driver only
  flushes and breaks.

The prompt names the enum variants `Connect`, `Download`, `Retry`, `Finish`
and the handler `handle_stream`, so the `Download` variant is driven by
`handle_stream`.

### 2.2 Session context

Add a private struct next to the enum:

```rust
struct DownloadSession {
    file: tokio::fs::File,
    total_size: Option<u64>,
    chunk_wait_timeout: Duration,
    response: Option<reqwest::Response>,
}
```

`response` is `Some(_)` only between a successful `handle_connect` and the
subsequent `handle_stream`. `DownloadSession` is an implementation detail and
does not need to be public.

### 2.3 Transition table

| Current state | Handler | Normal next state | Retry next state | Terminal next state |
| --- | --- | --- | --- | --- |
| `Connect { downloaded, attempts }` | `handle_connect` | `Download { downloaded, attempts: 0 }` | `Retry { downloaded, attempts: attempts + 1 }` | `Finish { downloaded, completed: false }` |
| `Download { downloaded, attempts }` | `handle_stream` | `Finish { downloaded, completed: true }` | `Retry { downloaded, attempts: attempts + 1 }` | `Finish { downloaded, completed: false }` |
| `Retry { downloaded, attempts }` | `handle_backoff` | `Connect { downloaded, attempts }` | n/a | n/a |
| `Finish { downloaded, completed }` | driver | break | n/a | n/a |

---

## 3. Implementation steps

### Step 1: Extend the state enum

Replace the current unit-only enum with the payload-carrying enum from
section 2.1. Keep the same enum name and visibility.

Check all existing references to `DownloadingState` (only the half-finished
`connect` currently uses it). Those references will be removed or rewritten in
Step 4.

### Step 2: Add `DownloadSession`

Add the private `DownloadSession` struct from section 2.2.

The driver owns one `DownloadSession` for the whole run. Handlers borrow it
mutably. This keeps `DownloadingState: Copy` while still allowing the response
to survive the transition from `handle_connect` to `handle_stream`.

### Step 3: Add `start_internal_refactored`

Add this method immediately after the existing `start_internal` (or before it,
but do not change the existing method):

```rust
#[allow(dead_code)] // remove once renamed to start_internal
async fn start_internal_refactored(&self) -> Result<(), DownloadRunError> {
    // 1. Preserve the status guard.
    let status = self.sender.borrow().clone().state;
    if !matches!(status, DownloadState::Running { .. }) {
        return Ok(());
    }

    // 2. Preserve total_size computation exactly.
    let total_size = if let Some(info) = &self.data.info {
        Some(info.size())
    } else {
        InPlaceSegment::get_total_size(self.client.clone(), &self.data).await
    };

    // 3. Preserve the early-completion path exactly.
    if let Some(size) = total_size
        && self.data.last_downloaded >= size
    {
        let _ = self.sender.send(DownloadStatus {
            total: total_size,
            downloaded: self.data.last_downloaded,
            state: DownloadState::Completed { size },
        });
        return Ok(());
    }

    // 4. Preserve file open flags.
    let file = tokio::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .open(&self.data.filepath)
        .await?;

    let mut session = DownloadSession {
        file,
        total_size,
        // TODO remains: make configurable later.
        chunk_wait_timeout: Duration::from_secs(10),
        response: None,
    };

    let mut state = DownloadingState::Connect {
        downloaded: self.data.last_downloaded,
        attempts: 0,
    };

    loop {
        state = match state {
            DownloadingState::Connect { downloaded, attempts } => {
                self.handle_connect(&mut session, downloaded, attempts).await?
            }
            DownloadingState::Download { downloaded, attempts } => {
                self.handle_stream(&mut session, downloaded, attempts).await?
            }
            DownloadingState::Retry { downloaded, attempts } => {
                self.handle_backoff(&mut session, downloaded, attempts).await?
            }
            DownloadingState::Finish { downloaded, completed } => {
                session.file.flush().await?;
                if completed {
                    let _ = self.sender.send(DownloadStatus {
                        downloaded,
                        total: Some(downloaded),
                        state: DownloadState::Completed { size: downloaded },
                    });
                }
                break;
            }
        };
    }

    Ok(())
}
```

Notes:

- Do not add a completion send after the loop; the `Finish` arm owns it.
- `#[allow(dead_code)]` is temporary so `cargo check` stays warning-free while
  `start_internal` still calls the legacy implementation.

### Step 4: Implement `handle_connect`

Signature:

```rust
async fn handle_connect(
    &self,
    session: &mut DownloadSession,
    downloaded: u64,
    attempts: u32,
) -> Result<DownloadingState, DownloadRunError>
```

Logic, preserving the legacy loop's connect phase:

1. Seek to the segment-relative position:
   ```rust
   let base_start = self.data.info.as_ref().map(|i| i.start).unwrap_or(0);
   session
       .file
       .seek(std::io::SeekFrom::Start(base_start + downloaded))
       .await?;
   ```
   This fixes the half-finished `connect`, which currently uses `base_start = 0`.

2. Build the `Range` header from the **live `downloaded` value**, not from
   `self.data.last_downloaded`. Either:
   - change `get_range_header` to
     `fn get_range_header(data: &InPlaceSegmentData, downloaded: u64) -> Option<String>`,
     or
   - compute it inline in `handle_connect`.

   Preferred: change the helper signature and update the half-finished `connect`
   call (or delete `connect`, see Step 8).

3. Build the request exactly as the legacy code does:
   - `self.client.get(&self.data.url)`
   - optional `Range`
   - optional `Cookie`, `Referer`, `User-Agent` from `self.data.headers`

4. Check the current command before any await:
   ```rust
   let mut cmd_rx = self.command_reciever.clone();
   match *cmd_rx.borrow() {
       SegmentCommand::Stop => {
           let _ = self.sender.send(DownloadStatus {
               total: session.total_size,
               downloaded,
               state: DownloadState::Stopped,
           });
           return Ok(DownloadingState::Finish { downloaded, completed: false });
       }
       SegmentCommand::Pause => {
           let _ = self.sender.send(DownloadStatus {
               total: session.total_size,
               downloaded,
               state: DownloadState::Paused,
           });
           session.file.flush().await?;
           return Ok(DownloadingState::Finish { downloaded, completed: false });
       }
       SegmentCommand::Nothing => {}
   }
   ```

5. Perform the cancel-safe handshake. Refactor the existing
   `fetch_res_unless_interrupt` so it distinguishes pause/stop from a real
   response. Suggested outcome type:

   ```rust
   enum FetchResponseOutcome {
       Response(reqwest::Response),
       Paused,
       Stopped,
   }
   ```

   `fetch_res_unless_interrupt` keeps its existing arguments, but returns
   `Result<FetchResponseOutcome, DownloadRunError>`:
   - `Pause`: publish `Paused`, flush the file, return `Paused`.
   - `Stop`: publish `Stopped`, return `Stopped`.
   - `Nothing` from `changed()`: return
     `Err(DownloadRunError::NetworkError("Unexpected command received".to_owned()))`
     exactly as the legacy code does.
   - `req.send()` error: return the reqwest error converted to
     `DownloadRunError::NetworkError`.

6. Map the fetch outcome:
   - `Paused | Stopped`:
     `Ok(DownloadingState::Finish { downloaded, completed: false })`
     (status already published and pause already flushed).
   - `Response(res)`: continue.

7. Apply the HTTP status checks exactly as the legacy code:
   ```rust
   let status = res.status();
   if range_header.is_some() && status != StatusCode::PARTIAL_CONTENT {
       return Err(DownloadRunError::ServerError(
           "Server does not support range requests".to_owned(),
       ));
   } else if !status.is_success() {
       return Err(DownloadRunError::NetworkError(format!(
           "Server returned HTTP {}",
           status
       )));
   }
   ```

8. Retry handling for a failed handshake:
   - If `attempts < self.retry_scheme.max_attempts`:
     ```rust
     return Ok(DownloadingState::Retry {
         downloaded,
         attempts: attempts + 1,
     });
     ```
   - Otherwise return
     `Err(DownloadRunError::NetworkError(e.to_string()))`.

9. On success:
   ```rust
   session.response = Some(res);
   Ok(DownloadingState::Download {
       downloaded,
       attempts: 0,
   })
   ```

Do **not** return `Retry` for pause/stop. That is a bug in the half-finished
`connect` and would reconnect after a user pause.

### Step 5: Implement `handle_stream`

Signature:

```rust
async fn handle_stream(
    &self,
    session: &mut DownloadSession,
    downloaded: u64,
    attempts: u32,
) -> Result<DownloadingState, DownloadRunError>
```

Logic:

1. Before entering `stream_chunks`, re-check the live command value. The watch
   receiver only notifies on changes made after `changed()` is armed, so a pause
   that arrives between the handshake and the stream must be caught here:
   ```rust
   match *self.command_reciever.borrow() {
       SegmentCommand::Pause => {
           let _ = self.sender.send(DownloadStatus {
               total: session.total_size,
               downloaded,
               state: DownloadState::Paused,
           });
           session.file.flush().await?;
           return Ok(DownloadingState::Finish {
               downloaded,
               completed: false,
           });
       }
       SegmentCommand::Stop => {
           let _ = self.sender.send(DownloadStatus {
               total: session.total_size,
               downloaded,
               state: DownloadState::Stopped,
           });
           return Ok(DownloadingState::Finish {
               downloaded,
               completed: false,
           });
       }
       SegmentCommand::Nothing => {}
   }
   ```

2. Take the response:
   ```rust
   let Some(mut res) = session.response.take() else {
       return Err(DownloadRunError::NetworkError(
           "No active response in Download state".to_owned(),
       ));
   };
   ```

3. Stream with the existing helper, using a local mutable copy of `downloaded`:
   ```rust
   let mut current_downloaded = downloaded;
   let result = self
       .stream_chunks(
           &mut res,
           &mut session.file,
           &mut current_downloaded,
           session.total_size,
           session.chunk_wait_timeout,
       )
       .await;
   ```

4. Map `stream_chunks` results exactly as the legacy loop:
   - `Ok(StreamChunkResult::Finished)`:
     ```rust
     Ok(DownloadingState::Finish {
         downloaded: current_downloaded,
         completed: true,
     })
     ```
   - `Ok(StreamChunkResult::Interrupted(cmd))`:
     - `Pause` => publish `Paused`
     - `Stop` => publish `Stopped`
     - `Nothing` => `unreachable!()` (preserve current assumption)
     - flush the file
     - return `Finish { downloaded: current_downloaded, completed: false }`
   - `Err(DownloadRunError::NetworkError(_) | DownloadRunError::TimeoutError)`
     when `attempts < self.retry_scheme.max_attempts`:
     ```rust
     Ok(DownloadingState::Retry {
         downloaded: current_downloaded,
         attempts: attempts + 1,
     })
     ```
   - Any other `Err(fatal_err)` or retry-limit exhaustion:
     `Err(fatal_err)`.
     Note: this preserves the legacy behavior where a `TimeoutError` at the
     retry limit propagates as `TimeoutError`, while a network error propagates
     as `NetworkError`.

5. The response is dropped when `handle_stream` returns. Do not attempt to reuse
   it after an interruption or error.

### Step 6: Implement `handle_backoff`

Signature:

```rust
async fn handle_backoff(
    &self,
    session: &mut DownloadSession,
    downloaded: u64,
    attempts: u32,
) -> Result<DownloadingState, DownloadRunError>
```

Logic:

```rust
let delay = self
    .retry_internal(attempts, session.total_size, downloaded)
    .await;
session.file.flush().await?;
tokio::time::sleep(delay).await;
Ok(DownloadingState::Connect {
    downloaded,
    attempts,
})
```

This preserves the legacy ordering: publish `Retrying`, flush, sleep, reconnect.
`retry_internal` calls `next_delay(attempts - 1)`, so callers must only create
`Retry` with `attempts >= 1`.

### Step 7: Adjust helper functions

1. `get_range_header` must accept the live `downloaded` value:

   ```rust
   fn get_range_header(
       data: &InPlaceSegmentData,
       downloaded: u64,
   ) -> Option<String> {
       match (&data.info, downloaded) {
           (Some(i), d) => Some(format!("bytes={}-{}", i.start + d, i.end)),
           (None, d) if d > 0 => Some(format!("bytes={}-", d)),
           _ => None,
       }
   }
   ```

   The current version silently uses `data.last_downloaded`, which is wrong
   after any in-run reconnect.

2. Keep `stream_chunks`, `get_next_chunk_or_cancel`, and `calculate_speed`
   unchanged. They already provide the required streaming, timeout, cancellation,
   progress, and speed behavior.

### Step 8: Remove or retire the half-finished `connect`

The existing `connect` is unused, contains `todo!()`, and duplicates the new
`handle_connect`. After `handle_connect` is implemented, either:

- **Recommended:** delete the half-finished `connect` so there is only one
  connect implementation, or
- leave it in place untouched, but update its `get_range_header(&self.data)`
  call if the helper signature changes.

Do not delete or modify the legacy `start_internal`.

---

## 4. Edge cases and invariants to verify

1. **Segment-relative seek:** always seek to
   `info.start.unwrap_or(0) + downloaded`, never just `downloaded`.
2. **Live range offset:** the `Range` header must use the current `downloaded`
   value, not `data.last_downloaded`.
3. **Pause/stop are terminal for the segment run:** return `Finish`, never
   `Retry`. `Stop` must ultimately return `Ok(())` so
   `DownloadItem::start` can delete the file.
4. **Pause flush:** flush the file on pause, both during connect and during
   streaming, before returning `Finish`.
5. **Missed watch notifications:** always read
   `*self.command_reciever.borrow()` before awaiting. `changed()` only reports
   changes that occur after the receiver is armed (see `tests/test_bug.rs`).
6. **Attempt counting:** reset `attempts` to `0` only after a successful
   handshake; increment before transitioning to `Retry`; never call
   `retry_internal(0, ...)`.
7. **Retryable errors:** only `NetworkError` and `TimeoutError` are retryable in
   the stream phase. `DiskError`, `ServerError`, `VerifyError`, and
   `AlreadyRunning` are fatal.
8. **HTTP status handling:** require `206` only when a `Range` header was sent.
   A non-ranged `200` is valid.
9. **Unknown total size:** `total_size == None` disables the early-completion
   check and propagates `None` to status updates.
10. **Zero-sized/complete file:** `last_downloaded >= size` handles a
    zero-length or already-complete file before opening the file, exactly as the
    legacy code does.
11. **Completion payload:** publish `Completed { size: downloaded }` with
    `total: Some(downloaded)`, not `total_size`, to preserve existing behavior.
12. **EOF semantics:** `StreamChunkResult::Finished` is considered success even
    for segmented runs, matching the legacy implementation. Do not add
    length-validation behavior in this refactor.
13. **Dead code:** the new state machine is intentionally unused until renamed.
    Add `#[allow(dead_code)]` only to `start_internal_refactored` if needed.
14. **Tests:** current integration tests are known to be out of date with the
    retry feature. Several call `DownloadManager::add_download` with five
    arguments while the current signature requires a sixth
    `retry_scheme: RetryScheme`. Do not use those failures as evidence that the
    refactor is wrong.

---

## 5. Files to modify

Only one production file is expected to change:

- `src-tauri/downloader/src/in_place_segment.rs`

No changes to:

- `download_item.rs`
- `download_manager.rs`
- `download_status.rs`
- `download_run_error.rs`
- `retry_scheme.rs`
- any integration test (unless the user explicitly asks for test adaptation)

---

## 6. Verification plan

Because `start_internal_refactored` is intentionally not called yet, verify in
stages.

### Stage 1: Compile the library

From `src-tauri/downloader`:

```bash
cargo check --lib
```

This must pass. Confirm there are no new warnings about unused new handlers; use
`#[allow(dead_code)]` on `start_internal_refactored` until the rename.

### Stage 2: Verify the legacy path is untouched

Review the diff and confirm:

- `start_internal` is byte-for-byte unchanged.
- `start()` still calls `start_internal`.
- `stream_chunks`, `get_next_chunk_or_cancel`, and `calculate_speed` are
  unchanged.
- The only helper signature change is `get_range_header`, plus the
  `fetch_res_unless_interrupt` return type used by the new `handle_connect`.

### Stage 3: Focused tests for the refactored path (recommended)

Add temporary `#[cfg(test)]` tests inside `in_place_segment.rs`, or a temporary
private test wrapper, that call `start_internal_refactored` directly. Cover at
minimum:

1. Successful single-stream download with `info: None`.
2. Successful segmented range download with `info: Some(SegmentInfo)`.
3. Handshake failure followed by a successful retry (use `RetryScheme` with a
   tiny base delay and `max_attempts >= 1`).
4. Stream interruption with `Pause`, asserting `DownloadState::Paused`, file
   flush, and `Ok(())`.
5. Stream interruption with `Stop`, asserting `DownloadState::Stopped` and
   `Ok(())`.
6. Retry exhaustion for a non-2xx handshake, asserting the expected
   `DownloadRunError`.

These tests can remain `#[cfg(test)]` and are not part of the public API.

### Stage 4: Temporary rename smoke test

In a scratch working tree, rename:

```rust
async fn start_internal(...)          // legacy
async fn start_internal_refactored(...) // new
```

to:

```rust
async fn start_internal_legacy(...)
async fn start_internal(...)          // new implementation
```

Then run whatever integration tests are currently compiling after accounting for
the known broken test signatures. Revert the rename afterward. This is the final
confidence check before the user performs the one-line rename.

---

## 7. Non-goals

- Do not fix the unrelated test suite signature drift.
- Do not change retry scheduling semantics.
- Do not change segment splitting, verification, manager behavior, or status
  aggregation.
- Do not change the command channel structure or the pause/resume lifecycle in
  `DownloadItem` / `DownloadManager`.
- Do not replace `start_internal` in this task.
