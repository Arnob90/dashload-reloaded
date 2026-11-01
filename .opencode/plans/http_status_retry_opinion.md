# Opinion: HTTP error statuses are never retried in `InPlaceSegment`

## Verdict

**Yes, the bug exists.** A response that successfully completes the HTTP
transport but carries a retryable error status (408, 425, 429, 500, 502, 503,
504) is converted directly into a terminal `DownloadRunError`. No backoff, no
reconnect, no increment of `attempts`. The first such response ends that
segment permanently.

This is not a regression introduced by the state-machine refactor. The previous
monolithic `start_internal` loop had the identical ordering, so the refactor
faithfully preserved an existing defect.

---

## Evidence

All references are to `src-tauri/downloader/src/in_place_segment.rs` at the
time of writing.

### 1. The retry budget only wraps the transport handshake

`handle_connect` (line 388) applies the retry guard **only** to the result of
`fetch_res_unless_interrupt` (lines 445-469):

```rust
let res = match self
    .fetch_res_unless_interrupt(req, cmd_rx, session.total_size, downloaded, &mut session.file)
    .await
{
    Ok(FetchResponseOutcome::Response(res)) => res,
    Ok(FetchResponseOutcome::Paused | FetchResponseOutcome::Stopped) => {
        return Ok(DownloadingState::Finish { downloaded, completed: false });
    }
    Err(_e) if attempts < self.retry_scheme.max_attempts => {
        return Ok(DownloadingState::Retry { downloaded, attempts: attempts + 1 });
    }
    Err(e) => return Err(e),
};
```

`fetch_res_unless_interrupt` (line 257) only ever fails on `req.send()` errors
(connreset, DNS, TLS) or an unexpected watch notification. A `503` is a
*successful* send: it is returned as `Ok(FetchResponseOutcome::Response(res))`
and leaves the `match` above on the non-retry path.

### 2. Status classification returns `Err` unconditionally

Immediately afterwards (lines 471-481):

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

Neither branch consults `attempts`, `retry_scheme`, or `DownloadingState::Retry`.
`start()` (line 173) then publishes `DownloadState::Failed`, and
`DownloadItem`'s aggregator (line 180) treats `Failed` as terminal. Nothing at a
higher layer restarts the segment.

### 3. Consequence depends on whether a `Range` header was sent

| Request shape | Response | Result |
| --- | --- | --- |
| Segmented or resumed (`Range` header present) | any non-206, including 408/429/500/502/503/504 | `ServerError("Server does not support range requests")`, fatal |
| Non-segmented fresh (`Range` absent) | 408/429/500/502/503/504 | `NetworkError("Server returned HTTP …")`, fatal |

So for multi-segment downloads it is actually **two** bugs in one line: the
status is not retried, *and* a transient `503` is misdiagnosed as a permanent
server capability problem. The error message actively misleads users and logs.
The same misclassification also swallows a genuine `416 Range Not Satisfiable`
into the "no range support" message.

### 4. The retry machinery is already there and would work

- `handle_backoff` (line 581) publishes `Retrying`, flushes, sleeps, and
  reconnects from the current offset.
- Reconnecting after an error status is safe: `handle_connect` re-seeks to
  `info.start.unwrap_or(0) + downloaded` and rebuilds the `Range` header from
  the live offset (lines 394-404).
- `get_range_header` (helper at the bottom of the file) already takes the live
  `downloaded` value.

Only the classification step is missing, which is why the behavior is easy to
mistake for intentional.

---

## Opinion

This is a genuine reliability bug, not a design preference. `RetryScheme` is
plumbed all the way from `DownloadManager` to the segment worker, and the worker
uses it for transport failures and mid-stream timeouts. Treating the entire
class of retryable HTTP statuses as fatal is inconsistent with that design and
produces exactly the failure mode users hit with flaky CDNs and rate-limited
hosts: a single `503` or `429` kills an otherwise healthy multi-hour download on
the first retryable response.

The current code is also internally inconsistent: a connection reset while
sending the request is retried, but a `503` returned on the very same request is
fatal. There is no principled distinction between those two — both are
transient server-side availability failures.

I would classify this as **high severity, low risk to fix**. The fix is local to
`handle_connect`; it does not alter the state machine shape, retry scheduling,
or any other file.

One caveat: do **not** retry everything non-2xx. `400`, `401`, `403`, `404`,
`405`, `410`, `416`, and the range-capability check must remain terminal (or be
reported as `ServerError`), otherwise a dead URL or a hard auth failure would
spend the whole retry budget before failing.

---

## Recommended fix (single file)

Insert the retryable-status classification **before** the range/`is_success`
checks, so that retryable statuses are drained by the same budget as transport
errors, and only genuinely permanent statuses reach `ServerError`.

```rust
let status = res.status();

// Retryable transient server failures share the connect-phase retry budget.
if is_retryable_http_status(status) {
    if attempts < self.retry_scheme.max_attempts {
        return Ok(DownloadingState::Retry {
            downloaded,
            attempts: attempts + 1,
        });
    }
    return Err(DownloadRunError::NetworkError(format!(
        "Server returned HTTP {}",
        status
    )));
}

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

session.response = Some(res);
Ok(DownloadingState::Download { downloaded, attempts: 0 })
```

With a small module-level helper:

```rust
/// Statuses that indicate a transient server-side condition worth retrying.
/// Deliberately excludes client errors such as 400/401/403/404/410/416.
fn is_retryable_http_status(status: StatusCode) -> bool {
    matches!(
        status,
        StatusCode::REQUEST_TIMEOUT        // 408
            | StatusCode::TOO_EARLY        // 425
            | StatusCode::TOO_MANY_REQUESTS // 429
            | StatusCode::INTERNAL_SERVER_ERROR // 500
            | StatusCode::BAD_GATEWAY      // 502
            | StatusCode::SERVICE_UNAVAILABLE // 503
            | StatusCode::GATEWAY_TIMEOUT  // 504
    )
}
```

Notes on the fix:

1. The guard order matters. Putting the retryable check first also fixes the
   misleading `ServerError("Server does not support range requests")` for a
   transient `503` on segmented requests.
2. Use `attempts + 1` exactly as the existing connect-error branch does, so the
   shared counter, `retry_internal(attempts)` (`attempts - 1` indexing), and the
   `Retrying` status publication all keep working unchanged.
3. Keep returning `NetworkError` on exhaustion so `start()` publishes `Failed`
   and the aggregate behavior is unchanged.
4. `500` is included even though it is sometimes permanent; with
   `max_attempts` defaulting to 5 the cost is bounded, and the alternative
   (treating every 500 as fatal) fails transient backend errors. Remove it from
   the list if you prefer a more conservative policy.
5. Optional enhancement: honour `Retry-After` on 429/503 by clamping or
   replacing the exponential delay. This is not required for correctness and
   would need `handle_backoff`/`Retry` to carry an optional duration, which is a
   larger change than the bug warrants. I would file it separately.

### Suggested tests

- Handshake returns `503`, `max_attempts = 1`, second response `206`/`200`: run
  succeeds, one `Retrying` status is observed, final state is `Completed`.
- Handshake returns `503` for every attempt: exactly `max_attempts + 1` total
  requests are made, final state is `Failed(NetworkError(..))`.
- Handshake returns `404`/`403`: exactly one request, final state
  `Failed(ServerError/NetworkError)` with no `Retrying` status.
- Segmented request returning `503` then `206`: verifies the status is retried
  instead of being misreported as a missing range capability.
- `RetryScheme { max_attempts: 0, .. }`: a single retryable status fails
  immediately without calling `retry_internal(0)`.

---

## Related observation (separate issue, lower priority)

`handle_connect` resets `attempts` to `0` on every successful handshake
(`DownloadingState::Download { attempts: 0 }`). This matches the old code, but
it means the retry budget only bounds *consecutive* failures. A server that
alternates a successful handshake with an immediate connection drop can retry
forever. That is a pre-existing design choice, not part of the HTTP-status bug,
and should be addressed independently if unbounded flapping is a concern.
