# Types with Changed Serialize/Deserialize/PartialEq Derives

This document tracks all type modifications involving `serde::Serialize`, `serde::Deserialize`, and `derivative::PartialEq` adds/removes during the unit test implementation plan.

## Added `PartialEq`

| File | Type | Reason |
|------|------|--------|
| `src/in_place_segment.rs` | `SegmentInfo` | Test assertions for `size()` |
| `src/retry_scheme.rs` | `ExponentialBackoff` | Serde round-trip comparison |
| `src/retry_scheme.rs` | `RetryScheme` | Serde round-trip comparison |
| `src/download_status.rs` | `DownloadState` | Table-driven `get_error` tests |
| `src/download_status.rs` | `DownloadStatus` | New struct; test infrastructure |
| `src/verifier.rs` | `VerifyError` | Serde tag-based deserialization |
| `src/verifier.rs` | `Verifier` | Internal struct; test deserialization |
| `src/verifier.rs` | `VerifierMaybe` | Optional wrapper tests |
| `src/download_data.rs` | `DownloadData` | Full serde round-trip tests |
| `src/request_headers.rs` | `RequestHeaders` | Round-trip, omission, invalid value tests |
| `src/length_getter.rs` | `LengthGettingErrors` | Test infrastructure (kept existing derives) |
| `src/info_getters.rs` | `FilepathGettingError` | Already present; tests added |

## Added `Serialize` / `Deserialize`

| File | Type | Changes |
|------|------|---------|
| `src/verifier.rs` | `VerifyError` | Removed `Deserialize` (std::io::Error can't be deserialized), kept `Serialize`; added `#[serde(tag = "kind", content = "payload")]` for externally-tagged JSON |
| `src/verifier.rs` | `Verifier` | Added `PartialEq + Serialize + Deserialize` |
| `src/verifier.rs` | `VerifierMaybe` | Added `PartialEq + Serialize + Deserialize` |
| `src/download_data.rs` | `DownloadData` | Added `PartialEq + Debug` (was `Serialize + Deserialize + Clone`; added `PartialEq` and `Debug`) |
| `src/in_place_segment.rs` | `SegmentInfo` | Added `PartialEq` (was `Debug + Clone + Serialize + Deserialize`) |

## No Structural Changes (Tests Only)

The following types received new unit tests but **no derive changes**:
- `src/misc.rs` — `clamp` function tests
- `src/get_error_message.rs` — `get_error_message` branch tests
- `src/download_run_error.rs` — `From` impls, `FailureDetails`, serde round-trip, Display
- `src/segmented_verifier.rs` — `parse_content_range` extraction + tests (refactor)
- `src/external_download_request.rs` — minimal/full payload serde tests (already had derives)
- `src/info_getters.rs` — `filesize_from_headers` extraction + tests (already had derives)

## Key Refactoring Notes

- `VerifyError` gained `#[serde(tag = "kind", content = "payload")]` to enable the tagged JSON shape required by the test in `serialize_io_error.rs` (payload is a plain string, not a struct).
- `DownloadData` had `PartialEq` added to enable `assert_eq!(data, back)` in the serde round-trip test; `Debug` added for `unwrap_err` compatibility.
- `ExponentialBackoff` gained `PartialEq` only (no derive changes to `RetryScheme` itself beyond what was needed for the test).
- All `#[cfg(test)] mod tests` blocks follow the existing inline convention from `download_options.rs`, `info_getters.rs`, and `download_data_factory.rs`.

