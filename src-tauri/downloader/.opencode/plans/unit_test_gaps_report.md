# Unit Test Gap Report — `dashload-downloader`

## 1. Executive summary

The crate's current unit-test coverage (inline `#[cfg(test)]`) lives in exactly
three files:

| File | Coverage today |
|---|---|
| `src/download_options.rs` | builder defaults, chaining, serde wire format |
| `src/info_getters.rs` | `sanitize_filename` (extensive) |
| `src/download_data_factory.rs` | `divide_into_segments` edge cases |

Everything else — **the retry backoff math, error-conversion `From` impls,
segmentation math, Range-header construction, HTTP-status classification, speed
calculation, `DownloadState` deserialization fallbacks, file-hash verification,
header round-tripping, single-message channel, session serialization, and the
status-aggregation logic** — is exercised (if at all) only through slow
wiremock integration tests, or not at all.

The single biggest structural gap: `src/in_place_segment.rs` contains several
pure, private functions (`is_retryable_http_status`, `get_range_header`,
`calculate_speed`) that are completely untestable from any integration test
and currently have **zero** tests. They are trivially unit-testable from an
inline `#[cfg(test)]` module with no refactoring needed.

Priority ranking of the gaps:

- **P0** — pure logic that can be tested with no infra changes at all
- **P1** — pure logic needing a local file (`tempfile`), still no network
- **P2** — logic needing a small refactor (extract pure core) for unit testing

---

## 2. P0 — Zero-infrastructure gaps (pure functions, no I/O)

### 2.1 `src/in_place_segment.rs` — untested private helpers  ⭐ highest value

These are `fn`s (not `async`, no I/O) and are only reachable from an inline
test module. Today they are exercised only implicitly by wiremock integration
tests, so regressions (e.g. off-by-one in range math) surface slowly.

| Function | Signature | Suggested tests |
|---|---|---|
| `is_retryable_http_status` (line 721) | `fn is_retryable_http_status(StatusCode) -> bool` | All seven retryable codes (408, 425, 429, 500, 502, 503, 504) return `true`; non-retryable (200, 206, 400, 401, 403, 404, 410, 416) return `false`. |
| `get_range_header` (line 735) | `fn get_range_header(&InPlaceSegmentData, u64) -> Option<String>` | 4 arms: segmented + fresh → `bytes={start}-{end}`; segmented + resumed → offset added to start (`bytes={start+d}-{end}`); non-segmented resumed → `bytes={d}-` (open-ended); non-segmented fresh (`d == 0`) → `None`. |
| `calculate_speed` (line 747) | `fn calculate_speed(u64, Duration) -> f64` | 1024 bytes over 1 s → 1024.0; 0 bytes → 0.0; sub-second durations (e.g. 512 B / 500 ms → 1024.0); large values. |
| `SegmentInfo::size` (line 26) | `pub fn size(&self) -> u64` | Single byte (`start == end`) → 1; multi-byte (`0..=49`) → 50. Watch for the overflow hazard when `end < start` (would panic under `Debug`, silently wrap in release — worth a doc test or explicit handling decision). |

How: add `#[cfg(test)] mod tests { use super::*; ... }` at the bottom of
`src/in_place_segment.rs`. `InPlaceSegmentData` is `pub` with `Clone` +
`Debug`, so constructing it in tests is trivial. No new dependencies.

### 2.2 `src/retry_scheme.rs` — `ExponentialBackoff::next_delay`

Currently **no tests at all**. It is the most-used untested pure function in the
crate (called every retry in production).

```rust
pub fn next_delay(&self, attempt: u32) -> Duration
```

Suggested tests (jitter disabled to make them deterministic):

- **Base case**: `next_delay(0) == base_delay` (factor^0 == 1).
- **Growth**: with `factor = 2.0`, `next_delay(1) == 2 * base`,
  `next_delay(2) == 4 * base`.
- **Cap**: `next_delay(1000)` never exceeds `max_delay`
  (assert `delay <= max_delay` — equality expected because the cap is applied
  before jitter).
- **Jitter bounds** (statistical, jitter enabled): loop 1000 times with
  `attempt = 0`; assert every result is in `[base, base + base/2 + 1ms]`
  (`jitter_range = max(delay_ms/2, 1)`, and `rand::random_range(0..n)` is
  half-open). This pins the contract without being flaky.
- **Defaults**: `ExponentialBackoff::default()` → base 1 s, max 30 s,
  factor 2.0, jitter true; `RetryScheme::default()` → `max_attempts == 5`,
  5 s / 10 s timeouts.
- **Serde round-trip**: `ExponentialBackoff` and `RetryScheme` both derive
  `Serialize/Deserialize` and cross the wire in `DownloadOptions` /
  `DownloadData`; add a round-trip test (the `download_options` serde test
  already pins the JSON shape, but the type-level round-trip is free).

How: inline `#[cfg(test)]` module in `src/retry_scheme.rs`.

### 2.3 `src/misc.rs` — `clamp`

Pure `fn`, zero tests. Three cases: below range → `min`; above → `max`;
in-range → identity. Add `#[cfg(test)]` module in `src/misc.rs`.

### 2.4 `src/download_run_error.rs` — `From` conversions and `FailureDetails`

No tests today.

- `From<std::io::Error>` (line 100): `io::Error::new(io::ErrorKind::PermissionDenied, "denied")`
  → `DownloadRunError::DiskError { message: "denied", path: None }`.
  (`PartialEq` is derived, so direct `assert_eq!`.)
- `From<reqwest::Error>` (line 110): construct a synthetic error via
  `reqwest::StatusCode::from_u16(503).unwrap().to_string()` is not needed —
  build `reqwest::Error` cheaply with
  `reqwest::Client::new().get("http://127.0.0.1:1/x").send().await.err()`
  inside a `#[tokio::test]` (connect-refused → no status, so
  `status_code == None`). The `Some(status)` arm needs a wiremock 500 response
  — acceptable, it's a one-liner mock, or leave to integration.
- `FailureDetails::new` (line 27): `timestamp_secs` is within
  `[now-5s, now+5s]` of `SystemTime::now()`; `failed_segment_index == None`;
  `attempts_made == 0`.
- `with_segment(3)` / `with_attempts(7)` chain and set the right fields.
- **Serde round-trip** for every `DownloadRunError` variant (tagged
  `kind`/`payload`) — `legacy_session_test.rs` covers deserialization of
  legacy shapes but the current wire format is only round-tripped for one
  variant (`ServerError` in `new_format_roundtrips`). A table-driven
  `cases: Vec<DownloadRunError>` test covers all six.
- **`Display` output** (thiserror strings): e.g.
  `NetworkError` renders as `"Network error (Some(503)): msg"`,
  `AlreadyRunning` renders exactly `"Download is already running"`. Cheap
  regression protection for the user-facing error text.

### 2.5 `src/download_status.rs` — `DownloadState::get_error`

One-line accessor, untested. Table-driven test: each variant → `None`,
except `Failed(details)` → `Some(&details.error)`. Add `#[cfg(test)]` module
here (or fold into 2.4).

### 2.6 `src/get_error_message.rs` — `get_error_message`

Untested pure function. Two branches:

- Object-shaped value (e.g. a `DownloadRunError`, which serializes as a
  tagged object): result must contain all original keys **plus**
  `message == given.to_string()`.
- Non-object value (e.g. a newtype wrapping a `String`): result is
  `{"kind": <val>, "message": <display>}`.

Test with `DownloadRunError::AlreadyRunning` for branch 1 and a custom
`#[derive(Serialize)] struct` serializing to a string/number for branch 2.

### 2.7 `src/external_download_request.rs` — serde shape

The listener POST body deserializes this struct; there is no test pinning the
JSON wire format (snake_case field names, `filename`/`filesize` optional →
`null` when absent). One test: `serde_json::from_str` of a minimal
`{"url":"...", "headers":{...}}` payload, and a round-trip. Note that
`RequestHeaders` has no explicit serde rename, so field names are already
snake_case — the test pins that for the frontend contract.

---

## 3. P1 — Local-filesystem gaps (uses `tempfile`, no network)

`tempfile` is already a dev-dependency.

### 3.1 `src/verifier.rs` — hash verification  ⭐ second highest value

`Verifier::verify` (private, but reachable through `VerifierMaybe`) is core
post-download safety logic with **no direct tests**.

Suggested tests in `#[cfg(test)]` module of `src/verifier.rs`:

- **Pass**: write known bytes (e.g. `"hello world"`) to `tempfile::NamedTempFile`,
  build `VerifierMaybe::new(HashAlgo::Sha256, expected)` with a hardcoded
  SHA-256 hex digest; `verify_with_hash(path)` → `Ok(true)`.
  Repeat for `Md5` and `Sha512` (three algorithms, table-driven).
  Expected digests for a fixed fixture are stable constants — compute once,
  hardcode.
- **Mismatch**: wrong expected hash → `Err(VerifyError::HashMismatch {
  expected, actual })`; assert both fields (this is the error path
  `DownloadItem::start` maps to `DownloadRunError::VerifyError`).
- **Missing file**: `verify_with_hash` on a nonexistent path →
  `Err(VerifyError::IOError(..))` — document the "never panics" contract from
  the doc comment (line 105–108).
- **Large file / chunk-boundary**: a file larger than one 8 KB read
  (e.g. 100 KB) to exercise the chunked-read loop; known digest as constant.
- **`VerifierMaybe::none()`**: `verify` / `verify_with_hash` → `Ok(true)`
  without touching the filesystem (assert on a nonexistent path — if it
  touched the file it would error).
- **Accessors**: `algorithm()`, `expected_hash()`, `is_some()` for both the
  `Some` and `None` cases.

### 3.2 `src/serialize_io_error.rs` — `serialize_io_error`

`std::io::Error` isn't serializable by default; this function is the custom
serializer used by `VerifyError::IOError`. Test by serializing
`VerifyError::IOError(io::Error::new(ErrorKind::NotFound, "nope"))` and
asserting the JSON payload is the exact string `"nope"` (i.e.
`{"kind":"IOError","payload":"nope"}` under serde's default derive).

### 3.3 `src/download_data_factory.rs` — `deserialize_datas`

`divide_into_segments` is covered, but `deserialize_datas` (line 120) — the
session-load path — has no test. Two tests with `tempfile`:

- Write a small `{"id": {...DownloadData...}}` JSON map (build via
  `serde_json::to_value` on one `DownloadData` constructed with
  `new_normal`) → `deserialize_datas(&path)` returns 1 entry with equal data.
- Write invalid JSON → `Err(DataSerializationError::DeserializationError(_))`.
- Missing file → `Err(DataSerializationError::IoError(_))`.

### 3.4 `src/download_data.rs` / full `DownloadData` serde round-trip

No test directly round-trips `DownloadData` (the persistence unit). Build one
via `DownloadDataFactory::new_segmented(...)` with 2 segments,
`serde_json::to_string` → `from_str`, `assert_eq!` (`DownloadData` is `Clone`;
add `PartialEq` derive or compare field-by-field). This pins the on-disk
session format that `legacy_session_test.rs` only touches via
`DownloadState::Failed`.

---

## 4. P2 — Needs a small refactor, then pure unit tests

These are valuable but require extracting a pure core before they can be
tested without spinning up servers. Recommend as follow-up work, ordered by
value.

### 4.1 `src/segmented_verifier.rs` — `Content-Range` parsing (highest P2 value)

`verify_segmented` (line 49) has ~50 lines of header-parsing logic
(lines 74–122) interleaved with HTTP. Refactor:

```rust
/// Extracts (start, end, total) from a Content-Range header value.
pub(crate) fn parse_content_range(value: &str) -> Result<(u64, u64, u64), SegmentVerificationError>
```

Then unit-test it table-driven with no server:

- `"bytes 0-0/1000"` → `(0, 0, 1000)` (the canonical case)
- `"bytes 5-9/100"` → `(5, 9, 100)`
- `"bytes 0-99/*"` (unknown total) → error (`ServerNotRangable`)
- `"bytes 0-0"` (missing `/total`) → error
- `"items 0-0/10"` (wrong unit) → error
- `"bytes 00-00/1000"` (leading zeros) — document behavior
- malformed garbage → error

The end-validation logic (`start != 0 || returned_end != 0 || total == 0`,
`end >= total_size` → `RangeError`) can also move into a pure
`fn validate_probe(...) -> Result<(), SegmentVerificationError>` if desired.
The existing wiremock segmented tests keep covering the network layer.

### 4.2 `src/length_getter.rs` — `get_length_from_response`

Takes a `reqwest::Response` directly, but constructing one without a server is
impractical. Two options:

- **Preferred**: extract the parsing to
  `pub(crate) fn length_from_header(value: &str) -> Result<u64, LengthGettingErrors>`
  and unit-test it: `"123"` → `Ok(123)`, `""` → `IllBehavedServer`,
  `"12a"` → `IllBehavedServer`, `"-1"` → `IllBehavedServer` (u64 parse
  rejects negatives), missing header → `LengthlessServerError` (covered by
  calling with `None` in a split `Result` signature, or leave to the existing
  integration tests).
- `get_length`'s HEAD→GET fallback is already integration-covered; leave it.

### 4.3 `src/request_headers.rs` — `to_header_map` / `from_header_map` round-trip

No tests. Pure, no server needed:

- **Round-trip**: build `HeaderMap` with cookie/referer/user-agent,
  `from_header_map` → struct fields match; then `to_header_map` → map
  contains exactly the three canonical headers with lowercased names
  (`cookie`, `referer`, `user-agent`).
- **Omission**: all-`None` struct → empty map (no headers inserted).
- **Invalid value**: `user_agent: Some("bad\0header".to_string())` →
  `to_header_map()` returns `Err(InvalidHeaderValue)` — pin the contract
  that garbage from a browser extension is rejected, not injected.
- **`Default`**: `user_agent` is the Firefox ESR string, cookies/referrer
  `None`.

### 4.4 `src/download_item.rs` — `get_status` aggregation (highest P2 value)

`DownloadItem::get_status` (line 200) implements non-trivial precedence logic
(failure > completed > paused > stopped > all-retrying > running) plus speed
summing and the `Verifying`/`VerifyFailed` override for completed items
(lines 282–288). It's exercised by integration tests but only incidentally.

Refactor: extract

```rust
pub(crate) fn aggregate_status(
    statuses: &[DownloadStatus],
    retry_scheme: &RetryScheme,
    verify_state: &VerificationState,
) -> DownloadStatus
```

and unit-test the precedence table without a `DownloadItem`:

- any `Failed` → aggregate `Failed` (even if others `Running`)
- all `Completed` + verify `Verifying` → `Verifying`
- all `Completed` + verify `Verified` → `Completed` (size = sum of
  downloaded)
- all `Completed` + verify `VerifyFailed(d)` → `Failed(d)`
- mix `Paused` + `Running` → `Paused`
- any `Stopped` + others running → `Stopped`
- all `Retrying` → `Retrying` with summed `attempt` and `max` of
  `wait_secs`
- `total: None` on any segment → aggregate `total == None`
- speed sums across running segments

This also makes the "echo each segment's own status" invariant in
`DownloadItem::stop` (lines 333–342) easier to reason about.

### 4.5 `src/message_pass.rs` — single-message channel

Small module, zero tests. `#[tokio::test]` cases (no infra needed):

- send → `wait_for_message` returns `Some(msg)`, second wait suspends
  (use `tokio::time::timeout(100ms, ...)` → elapses).
- **Overwrite semantics**: send "a", send "b" before any receive →
  `wait_for_message` returns "b" (last write wins — the doc contract).
- Two sequential messages are delivered one after another.

### 4.6 `src/info_getters.rs` — `get_filesize` header parsing

The URL-filename path is integration-tested; `get_filesize` (line 139) is a
chain of `.ok()`s. Same refactor pattern as 4.2: extract
`pub(crate) fn filesize_from_headers(headers: &HeaderMap) -> Option<u64>`
and unit-test: valid `Content-Length: 1234` → `Some(1234)`; missing header →
`None`; `"abc"` → `None`. Cheap and pins the "silent None" behavior.

---

## 5. Explicitly out of scope (network-bound — keep in integration tests)

No unit tests are feasible or worth attempting for:

- `InPlaceSegment::start_internal` / `handle_connect` / `handle_stream` /
  `handle_backoff` state machine — already covered by
  `wiremock_retry*.rs`, `wiremock_error_scenarios.rs`,
  `wiremock_pause_resume.rs`.
- `DownloadManager` lifecycle methods (`add_download`, `resume`, `retry`,
  `stop`, `delete`) — covered by `wiremock_session_persistence.rs`,
  `download_resume_test.rs`.
- `get_download_info`, `get_filename_from_url`, `get_length` network flows —
  covered by `filename_integration_test.rs` and the wiremock suite.
- `listener.rs` axum server — a `tower::ServiceExt` round-trip test of the
  router would be *possible* (no real socket needed) but low value; the
  handlers are two lines. Optional bonus, not recommended.

---

## 6. Implementation plan

All steps are independent and land as small, individually-verifiable commits.
No changes to `Cargo.toml` are required (all needed dev-deps exist:
`tempfile`, `serde_json`, `tokio`, `wiremock`).

| Step | File(s) touched | New tests | Verify |
|---|---|---|---|
| 1 | `src/in_place_segment.rs` | 3–4: `is_retryable_http_status`, `get_range_header` (4 arms), `calculate_speed`, `SegmentInfo::size` | `cargo test --lib in_place_segment` |
| 2 | `src/retry_scheme.rs` | ~8: base/growth/cap/jitter-bounds/defaults/serde round-trip | `cargo test --lib retry_scheme` |
| 3 | `src/misc.rs`, `src/get_error_message.rs` | clamp (3 cases); get_error_message both branches | `cargo test --lib misc --lib get_error_message` |
| 4 | `src/download_run_error.rs`, `src/download_status.rs` | From impls, FailureDetails builder, full-variant serde table, Display strings, `get_error` | `cargo test --lib download_run_error --lib download_status` |
| 5 | `src/verifier.rs`, `src/serialize_io_error.rs` | pass/mismatch/IOError/large-file/none/accessors (use `tempfile::NamedTempFile`); io-error-as-string | `cargo test --lib verifier` |
| 6 | `src/request_headers.rs` | round-trip, omission, invalid value, Default | `cargo test --lib request_headers` |
| 7 | `src/download_data_factory.rs` (+ `DownloadData` in `src/download_data.rs`) | `deserialize_datas` valid/invalid/missing; full `DownloadData` round-trip | `cargo test --lib download_data` |
| 8 | `src/external_download_request.rs` | serde shape + minimal payload | `cargo test --lib external_download_request` |
| 9 *(refactor)* | `src/segmented_verifier.rs` | extract `parse_content_range`; ~7 table cases | `cargo test --lib segmented_verifier` **and full `cargo test`** (wiremock suite must still pass) |
| 10 *(refactor)* | `src/length_getter.rs`, `src/info_getters.rs` | extract header-parsing helpers; ~6 cases each | same as step 9 |
| 11 *(refactor)* | `src/message_pass.rs` | overwrite/sequential/timeout semantics | `cargo test --lib message_pass` |
| 12 *(refactor)* | `src/download_item.rs` | extract `aggregate_status`; ~9 precedence cases | `cargo test --lib download_item` **and full `cargo test`** |

### Conventions to follow

- Place tests in inline `#[cfg(test)] mod tests` (matches existing style in
  `download_options.rs`, `info_getters.rs`, `download_data_factory.rs`) —
  this is what makes private fns in `in_place_segment.rs` testable without
  any visibility change.
- For refactors (steps 9–12), keep the existing public signatures identical;
  the extracted core is `pub(crate)`.
- Hardcoded digests for verifier tests: compute once with any tool, commit as
  `const` (precedent: `TEST_FILE_SHA256` in `tests/wiremock_helpers.rs`).
- Jitter test uses statistical bounds (1000 iterations, half-open range
  `[base, base + jitter_range]`) to stay deterministic without seeding —
  `rand` is not seedable here without a bigger dep.
- After every step run full `cargo test` to confirm the integration suite is
  unaffected (it shares `tests/wiremock_helpers.rs` constants).

### Risk notes

- Step 4's `From<reqwest::Error>` test needs `#[tokio::test]` + a
  connect-refused URL; it is deterministic on Linux (connection refused) but
  keep `max_attempts == 0` semantics in mind — the conversion itself doesn't
  retry.
- Step 12's refactor touches `DownloadItem::get_status`, which `stop()`
  (line 336) and `snapshot_data()` (line 308) call — run the pause/resume
  and session-persistence wiremock tests specifically after this step.
- `SegmentInfo::size` underflows when `end < start`; tests in step 1 should
  document current behavior (debug panic / release wrap) so a decision can
  be made rather than silently locking the behavior in.
