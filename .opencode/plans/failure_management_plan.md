# Failure Management Critique, Architecture, and Implementation Plan

## Executive Summary

An in-depth analysis of the `dashload-reloaded` codebase confirms the suspicion: **the download manager current architecture handles failure internally, but severely under-reports and poorly surfaces failures to the user interface.**

While the Rust backend captures rich, typed errors (`DownloadRunError` with `NetworkError`, `ServerError`, `DiskError`, `VerifyError`, `TimeoutError`), these errors are largely lost or obscured as they travel through the status polling pipeline and into the frontend UI layer. When a download fails, the user sees only a generic red "Failed" badge with no error message, no diagnostic details, no failed segment context, and no actionable retry button. Furthermore, if a background status or data fetch query fails, the main download table completely unmounts and displays a static "Loading downloads..." message indefinitely.

This document presents a comprehensive critique of the existing failure management pipeline, designs a target architecture for robust error reporting, and details a step-by-step implementation plan.

---

## 1. Critique of Current Failure Management System

### 1.1 Backend Gaps (Rust / Tauri)

1. **Information Loss During Status Aggregation (`DownloadItem::get_status`)**:
   - When one segment fails, `DownloadItem::get_status()` sets the overall state to `DownloadState::Failed(err)`.
   - However, if multiple segments fail or if a failure is intermittent, only the *first* encountered error is retained.
   - There is no metadata attached to the failure:
     - No timestamp of when the failure occurred.
     - No indication of *which segment* (e.g., Segment 2 of 4) triggered the failure.
     - No HTTP status code (e.g., `404 Not Found`, `403 Forbidden`, `429 Too Many Requests`, `503 Service Unavailable`).
     - No information on how many automatic retries were attempted before giving up.

2. **Inflexible Automatic Retry Engine**:
   - Retries occur inside `InPlaceSegment::start_internal()` via `RetryScheme`. However, when max attempts are exceeded, the segment transitions directly to `DownloadState::Failed`.
   - `DownloadManager::retry(id)` exists on the backend to manually restart a failed/stopped download, **but there is no Tauri IPC command exposed to the frontend to call `retry`**.

3. **In-Memory Volatility of Failed State**:
   - If a download fails and the application is restarted, `DownloadManager::reconstruct` restarts saved session items into `DownloadState::Running` without preserving the previous failed state or error context.

4. **Inconsistent Header & Filesize Probing Errors**:
   - Range verification failures (`segmented_verifier::verify_segmented`) during `add_download` return a generic `ManagerError::SegmentedNotSupported(reason)`. The user is not told *why* range requests failed (e.g., missing `Accept-Ranges` header vs `200 OK` returned instead of `206 Partial Content`).

---

### 1.2 Frontend Gaps (React / Mantine / TanStack Query)

1. **Discarded Failure Payload in UI Rendering**:
   - In `DownloadTable` (`downloadtable.tsx`), line 155-165, the status column maps `status.kind` to a badge color: `Failed: "red"`.
   - **The actual error payload (`status.payload`) is completely ignored.** The user sees the word "Failed" in a red pill, but has no way of knowing whether it failed due to a full disk, a lost internet connection, an expired session cookie, or a checksum mismatch.
   - The action button column for `Failed` rows returns `null` (line 136). The user cannot click "Retry" or "Resume" — their only option is to delete the download and re-add it from scratch.

2. **Catastrophic Table Unmounting on Query Error**:
   - In `useTableEntries` (`tableEntry.ts` lines 103-108):
     ```typescript
     if (
       isPendingDownload ||
       isPendingStatus ||
       statusGettingError != null ||
       downloadDataGettingError != null
     ) {
       return null;
     }
     ```
   - If `getStatus` or `getData` fails even once (e.g. backend lock contention or transient JSON parse issue), `useTableEntries` returns `null`. The entire `DownloadTable` vanishes and is replaced by `<Text p="md">Loading downloads...</Text>`.

3. **No Retrying / Verifying Details Rendered**:
   - The backend produces `Retrying { attempt, max_attempts, wait_secs }` and `Verifying` states.
   - Neither state is styled in the `colorMap` of `downloadtable.tsx` or `downloadDetails.tsx` (falling back to unstyled gray).
   - The retry count and countdown timer (`wait_secs`) are never rendered in the UI.

4. **Missing Error Tab in Download Details Pane**:
   - `DownloadDetailsPane` (`downloadDetails.tsx`) has tabs for "Overview", "Segments", and "HTTP Context".
   - When a download fails, there is no "Error Log" or "Diagnostics" tab to inspect the error history, failure time, or segment error breakdown.

---

## 2. Proposed Architecture for Enhanced Failure Management

```
┌──────────────────────────────────────────────────────────────────────────────────┐
│                                 RUST BACKEND                                     │
│                                                                                  │
│  ┌───────────────────────┐   ┌──────────────────────┐   ┌─────────────────────┐  │
│  │ Enhanced Error Types  │   │ Failure Context Meta │   │ DownloadItem Status │  │
│  │  - NetworkError(code) │   │  - Failed segment #  │   │  - Aggregates all   │  │
│  │  - ServerError(code)  │   │  - Timestamp         │   │    segment errors   │  │
│  │  - DiskError(path)    │   │  - Attempts exhausted│   │  - Retains error    │  │
│  │  - VerifyError        │   │  - Raw underlying msg│   │    history in state │  │
│  └───────────┬───────────┘   └──────────┬───────────┘   └──────────┬──────────┘  │
│              │                          │                          │             │
│              └──────────────────────────┴──────────────────────────┘             │
│                                         │                                        │
│                           Tauri Command: retry_download                          │
└─────────────────────────────────────────┬────────────────────────────────────────┘
                                          │ IPC (JSON)
┌─────────────────────────────────────────▼────────────────────────────────────────┐
│                               FRONTEND (REACT)                                   │
│                                                                                  │
│  ┌────────────────────────────────────────────────────────────────────────────┐  │
│  │ Extended DownloadState Schema (supports Rich Failure Details)              │  │
│  └──────────────────────────────────────┬─────────────────────────────────────┘  │
│                                         │                                        │
│        ┌────────────────────────────────┼────────────────────────────────┐       │
│        ▼                                ▼                                ▼       │
│  ┌────────────────────┐   ┌───────────────────────────┐   ┌───────────────────┐  │
│  │ Download Table     │   │ Download Details Pane     │   │ Action Handlers   │  │
│  │ - Red badge w/ icon│   │ - Dedicated "Error" Tab   │   │ - Retry Button    │  │
│  │ - Tooltip / Popover│   │ - Copyable stack trace    │   │   invokes command │  │
│  │ - Failure progress │   │ - Per-segment error list  │   │ - Gentle query    │  │
│  │   bar styling      │   │ - Actionable suggestions  │   │   error fallback  │  │
│  └────────────────────┘   └───────────────────────────┘   └───────────────────┘  │
└──────────────────────────────────────────────────────────────────────────────────┘
```

### 2.1 Core Architectural Principles

1. **Rich Error Payload**:
   Error payloads must contain human-readable summaries, machine-readable category codes, optional HTTP status codes, timestamp of occurrence, and segment context.

2. **Actionable Recovery (Manual Retry)**:
   Users must be able to manually retry any failed or stopped download from the UI with a single click (or context menu option).

3. **Resilient Frontend Rendering**:
   A query error in background status polling should never clear the UI or block user interaction. Transient errors should display an alert bar while preserving the last known valid state (`placeholderData`).

4. **Transparent Diagnostic Context**:
   The `DownloadDetailsPane` will feature a dedicated "Error Log" tab whenever `entry.status.kind === "Failed"`, displaying the exact cause, failed segment, timestamp, and suggested resolution (e.g. "Disk full: check free space on drive C:").

---

## 3. Step-by-Step Implementation Plan

### Step 1: Backend Error Model & IPC Commands

**Files to modify:**
- `src-tauri/downloader/src/download_run_error.rs`
- `src-tauri/downloader/src/download_status.rs`
- `src-tauri/downloader/src/download_item.rs`
- `src-tauri/downloader/src/download_manager.rs`
- `src-tauri/src/commands.rs`

**Changes:**
1. Enhance `DownloadRunError` struct/enum:
   ```rust
   #[derive(Debug, Error, Clone, PartialEq, Serialize, Deserialize)]
   #[serde(tag = "kind", content = "payload")]
   pub enum DownloadRunError {
       #[error("Network error ({status_code:?}): {message}")]
       NetworkError {
           message: String,
           status_code: Option<u16>,
       },
       #[error("Server error ({status_code:?}): {message}")]
       ServerError {
           message: String,
           status_code: Option<u16>,
       },
       #[error("Disk I/O error: {message} (path: {path:?})")]
       DiskError {
           message: String,
           path: Option<String>,
       },
       #[error("Download is already running")]
       AlreadyRunning,
       #[error("Failed to verify completed download: expected {expected}, got {actual}")]
       VerifyError {
           expected: String,
           actual: String,
       },
       #[error("Timed out waiting for next chunk after {secs}s")]
       TimeoutError {
           secs: u64,
       },
   }
   ```
2. Add error metadata wrapper `FailureDetails`:
   ```rust
   #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
   pub struct FailureDetails {
       pub error: DownloadRunError,
       pub timestamp_secs: u64,
       pub failed_segment_index: Option<usize>,
       pub attempts_made: u32,
   }
   ```
3. Expose `retry_download` command in `src-tauri/src/commands.rs`:
   ```rust
   #[tauri::command]
   async fn retry_download(
       state: State<'_, AppState>,
       id: &str,
   ) -> Result<String, ApiError<ManagerError>> {
       Ok(state.download_manager.retry(id).await?)
   }
   ```
   Register `retry_download` in `get_handlers()`.

---

### Step 2: Frontend Types & API Client Updates

**Files to modify:**
- `src/code/typesOfBackend.ts`
- `src/code/apiCommunicate.ts`
- `src/code/useDownloads.ts`

**Changes:**
1. Update `DownloadRunErrorSchema` and `DownloadStateSchema` in `typesOfBackend.ts` to match the enriched Rust structs.
2. Add `retryDownload(id: string)` in `apiCommunicate.ts`.
3. Add `useRetryDownload()` hook in `useDownloads.ts`.

---

### Step 3: Resilient Data Merging & UI Recovery (`tableEntry.ts`)

**Files to modify:**
- `src/components/downloadtable/tableEntry.ts`

**Changes:**
1. Prevent UI unmounting when status queries fail:
   - Use `statusDatas` and `downloadDatas` from React Query `placeholderData` even if `error` is present.
   - Return cached `entries` during transient query errors instead of returning `null`.
2. Map rich failure details into `TableEntry`:
   ```typescript
   export interface TableEntry {
     // ... existing fields
     failureDetails: FailureDetails | null;
   }
   ```

---

### Step 4: Download Table & Actions Enhancement

**Files to modify:**
- `src/components/downloadtable/downloadtable.tsx`

**Changes:**
1. Update Status Badge:
   - For `Failed`, render a red Badge with a clickable Tooltip or Info icon revealing the failure message.
   - For `Retrying`, render an orange Badge showing `Retrying (x/y)`.
2. Add Action Buttons for `Failed` & `Stopped` rows:
   - Render a **Retry** button (`RotateCcw` or `RefreshCw` icon) in the Actions column for rows in `Failed` or `Stopped` state.
   - Wire the button to `useRetryDownload()`.
3. Add "Retry Download" item in the right-click Context Menu.

---

### Step 5: Detailed Error Diagnostics Pane (`downloadDetails.tsx`)

**Files to modify:**
- `src/components/downloadtable/downloadDetails.tsx`

**Changes:**
1. Add an **"Error & Diagnostics"** tab that appears automatically when `entry.status.kind === "Failed"`.
2. Display:
   - Primary Failure Category (e.g., `Disk I/O Error`, `Server Error 403`).
   - Detailed Message & Path/URL context.
   - Suggested Remediation (e.g., "Check folder write permissions or free disk space").
   - Segment-level error breakdown.
   - "Copy Error Details" button for easy reporting/troubleshooting.

---

## 4. Edge Cases & Risk Mitigation

| Edge Case | Risk | Mitigation |
|---|---|---|
| Persistent state deserialization | Legacy `downloads.json` files with old `DownloadRunError` format fail to load on startup | Add fallback/serde default handling or version tag in JSON format |
| Concurrent retry clicks | User rapidly clicks Retry while previous runner handle is shutting down | `DownloadManager::retry` already acquires `op_lock(id)` and awaits `runner_handle.remove(id)` |
| Segment-level retry race | One segment fails while another is retrying | Aggregated status checks `any_failed` and preserves error snapshot until manual retry |
| Query error polling flood | Toast notifications flood screen during network disconnect | Debounce error notifications or show a single banner |

---

## Verification Strategy

1. **Unit Tests (Rust)**:
   - Test `DownloadRunError` serialization and deserialization.
   - Test `DownloadManager::retry` on a mock failing download.
2. **Integration Tests**:
   - Simulate 404 / 500 HTTP response and verify `ServerError` payload structure.
   - Simulate read-only file path write and verify `DiskError` payload structure.
3. **Frontend Component Tests**:
   - Verify `DownloadTable` renders Retry button for `Failed` rows.
   - Verify `DownloadDetailsPane` displays the Error tab when selected row is failed.
