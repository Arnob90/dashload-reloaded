# Plan: Align `typesOfBackend.ts` + `apiCommunicate.ts` with the Tauri command API

Source of truth: `src-tauri/src/commands.rs` (+ the `downloader` crate types it exposes).

---

## 1. Backend API inventory (`src-tauri/src/commands.rs`)

| # | Command | Args (JS key → Rust) | Returns | Error payload |
|---|---------|----------------------|---------|---------------|
| 1 | `add_download` | `url: string`, `filepath: string`, `numberOfSegments: number`, `verifier: VerifierMaybe`, **`requestHeaders: RequestHeaders`** | `string` (download id) | `ApiError<ManagerError>` |
| 2 | `open_file` | – | `string \| null` (null = user cancelled) | – |
| 3 | `get_config_file` | – | `Config { auto_add_download: boolean }` | – |
| 4 | `get_status` | – | `Record<string, DownloadStatus>` | – |
| 5 | `get_item_status` | `id: string` | `DownloadStatus` | `ApiError<ManagerError>` |
| 6 | `get_download_metadata` | `url: string` | `DownloadInfo` | `ApiError<DownloadInfoGetError>` |
| 7 | `pause` | `id: string` | `null` (unit) | `ApiError<ManagerError>` |
| 8 | `resume` | `id: string` | `null` (unit) | `ApiError<ManagerError>` |
| 9 | `get_data` | – | `Record<string, DownloadData>` | – |
| 10 | `save_session` | – | `null` (unit) | `ApiError<DataSerializationError>` |

Not in the backend: `get_filename_from_path` (currently invoked by `apiCommunicate.ts` — dead command).

### Rust types referenced

- `RequestHeaders` (`downloader/src/request_headers.rs`): `{ cookies: Option<String>, referrer: Option<String>, user_agent: Option<String> }`
- `VerifierMaybe` (`downloader/src/verifier.rs`): `{ verifier: Option<Verifier> }`, `Verifier { to_match_hash: String, algo: HashAlgo }`
- `HashAlgo`: `Sha256 | Sha512 | Md5` (unit variants → plain strings)
- `DownloadStatus` (`downloader/src/download_status.rs`): `{ total: Option<u64>, downloaded: u64, state: DownloadState }`
- `DownloadState`: `#[serde(tag="kind", content="payload")]`
- `DownloadRunError`: `#[serde(tag="kind", content="payload")]`
- `ManagerError` (`downloader/src/download_manager.rs`): `IdDoesNotExist | DownloadCreationError(String) | OperationFailed(String) | NoFilesizeError`
- `DownloadInfoGetError` (`downloader/src/get_download_info.rs`): `RequestError(String) | UrlError(String)`
- `DataSerializationError` (`downloader/src/serialization_error.rs`): `IoError(String) | DeserializationError(String)`
- `DownloadData` / `InPlaceSegmentData` (`downloader/src/download_data.rs`, `in_place_segment.rs`)
- `ApiError<T>` (`src-tauri/src/api_error.rs`): `{ message: String, #[serde(flatten)] error: T }`
- `Config` (`src-tauri/src/config_file.rs`): `{ auto_add_download: bool }`

---

## 2. Verified wire formats

Confirmed empirically with a scratch serde project (`/tmp/opencode/serdecheck`) using the same
`tag`/`content` + `flatten` attributes as the backend:

```jsonc
// ApiError<ManagerError>  (flatten => error fields are siblings of "message")
{ "message": "boom", "kind": "DownloadCreationError", "payload": "why" }
{ "message": "boom", "kind": "IdDoesNotExist" }          // unit variant => no "payload"

// DownloadState
{ "kind": "Stopped" }                                     // unit variant => no "payload"
{ "kind": "Running",    "payload": { "download_speed": 1.5 } }
{ "kind": "Completed", "payload": { "size": 9 } }
{ "kind": "Failed",    "payload": { "kind": "NetworkError", "payload": "net" } }  // nested enum
{ "kind": "Paused" }
{ "kind": "Verifying" }

// DownloadStatus
{ "total": null, "downloaded": 3, "state": { "kind": "Paused" } }

// RequestHeaders (Option fields are always emitted, no skip_serializing_if)
{ "cookies": null, "referrer": "r", "user_agent": null }
```

Deserialization facts confirmed by the same experiment:
- Missing `Option` fields inside a struct **default to `None`** (serde allows `{"cookies":null}` → all `None`).
- A **missing top-level command argument** (e.g. omitting `requestHeaders` entirely) is still a
  hard Tauri "invalid args" failure, because the arg itself is required by the command signature.

---

## 3. Gaps in the current TypeScript (bugs to fix)

| # | Location | Problem | Impact |
|---|----------|---------|--------|
| G1 | `apiCommunicate.ts` `addDownload` | Does **not** send `requestHeaders` | `add_download` fails today: "invalid args" — downloads cannot be started |
| G2 | `typesOfBackend.ts` `InPlaceSegmentDataSchema` | Missing `headers: RequestHeaders` (added in Rust) | `get_data` zod parse fails → download table is empty/broken |
| G3 | `typesOfBackend.ts` `DownloadRunErrorSchema` | Requires a `message` field the Rust enum never emits | `Failed` download states fail to parse → `get_status` throws on any failed download |
| G4 | `typesOfBackend.ts` `ManagerErrorSchema` | Missing the `NoFilesizeError` variant | `add_download` errors are unparseable |
| G5 | `typesOfBackend.ts` | `ApiError` flatten shape not modelled; error schemas mix "enum" and "API error" concerns | Callers cannot rely on `err.message` + `err.kind` + `err.payload` consistently |
| G6 | `apiCommunicate.ts` | Missing wrappers for `get_config_file`, `get_item_status`, `save_session` | 3 of 10 commands are unreachable from the frontend |
| G7 | `apiCommunicate.ts` `getFilenameFromPath` | Invokes `get_filename_from_path`, which does not exist in `commands.rs` | Dead/broken function (a local `utils.ts` version is the one actually used) |
| G8 | `apiCommunicate.ts` `openFile` | `z.string()` rejects the `null` the backend returns on cancel | Cancel dialog → thrown ZodError instead of `null` |
| G9 | `apiCommunicate.ts` `pause` / `resume` | No error mapping | Raw Tauri error objects leak to the UI (`downloadtable.tsx` reads `err.message`) |
| G10 | `typesOfBackend.ts` | `Config` type/schema does not exist | Cannot type `get_config_file` |

---

## 4. Plan — `src/code/typesOfBackend.ts`

Keep the file's existing conventions (numbered sections, `zod` schema + `z.infer` type per Rust type,
1:1 naming with the Rust structs).

### 4.1 New section: Request headers

```ts
export const RequestHeadersSchema = z.object({
    cookies: z.string().nullable(),
    referrer: z.string().nullable(),
    user_agent: z.string().nullable(),
});
export type RequestHeaders = z.infer<typeof RequestHeadersSchema>;
```
> `user_agent` is snake_case on purpose: the Rust struct field is `user_agent`, and unlike command
> *arguments*, struct *fields* are not camelCase-converted by Tauri. The `null` allowance is required
> because Rust `Option<T>` serialises to `null` (no `skip_serializing_if`).

### 4.2 New section: API error wrapper

The Rust `ApiError<T>` flattens `T` next to `message`, so model it explicitly:

```ts
/** Rust: ApiError<T> { message: String, #[serde(flatten)] error: T } */
export function withApiMessage<T extends z.ZodTypeAny>(errorSchema: T) {
    return z.intersection(z.object({ message: z.string() }), errorSchema);
}
export type ApiError<T extends z.ZodTypeAny> = z.infer<ReturnType<typeof withApiMessage<T>>>;
```

Keep the *pure* error enums (matching the Rust enums) and derive the API-wrapped variants from them:
- `ManagerErrorSchema` / `ManagerApiErrorSchema` + types `ManagerError` / `ManagerApiError`
- `DownloadInfoGetErrorSchema` / `DownloadInfoGetApiErrorSchema` + types
- `DataSerializationErrorSchema` / `DataSerializationErrorApiErrorSchema` + types

### 4.3 Fix `DownloadRunErrorSchema` (G3)

Rust unit variants carry no payload and no message:

```ts
export const DownloadRunErrorSchema = z.discriminatedUnion("kind", [
    z.object({ kind: z.literal("NetworkError"), payload: z.string() }),
    z.object({ kind: z.literal("ServerError"),  payload: z.string() }),
    z.object({ kind: z.literal("DiskError"),     payload: z.string() }),
    z.object({ kind: z.literal("AlreadyRunning") }),
    z.object({ kind: z.literal("VerifyError") }),
]);
```
> `DownloadRunError` is **not** wrapped in `ApiError` — it is nested inside `DownloadState::Failed`,
> so it has no `message` field. Drop `message` here.

### 4.4 Fix `ManagerErrorSchema` (G4)

```ts
export const ManagerErrorSchema = z.discriminatedUnion("kind", [
    z.object({ kind: z.literal("IdDoesNotExist") }),
    z.object({ kind: z.literal("NoFilesizeError") }),
    z.object({ kind: z.literal("DownloadCreationError"), payload: z.string() }),
    z.object({ kind: z.literal("OperationFailed"), payload: z.string() }),
]);
```

### 4.5 Fix `DownloadInfoGetErrorSchema`

Keep as a pure enum (`RequestError` / `UrlError`, both with `payload: string`); the API-wrapped variant
adds `message`. (Today the schema accidentally contains both shapes in one object — splitting them makes
the nesting explicit and lets `addDownloadUrl.tsx` use the wrapped type.)

### 4.6 Add `DataSerializationErrorSchema`

```ts
export const DataSerializationErrorSchema = z.discriminatedUnion("kind", [
    z.object({ kind: z.literal("IoError"), payload: z.string() }),
    z.object({ kind: z.literal("DeserializationError"), payload: z.string() }),
]);
```

### 4.7 Fix `InPlaceSegmentDataSchema` (G2)

```ts
export const InPlaceSegmentDataSchema = z.object({
    url: z.string(),
    filepath: z.string(),
    last_downloaded: z.number(),
    info: SegmentInfoSchema.nullable(),
    headers: RequestHeadersSchema,        // <-- new
});
```

### 4.8 Add `ConfigSchema` (G10)

```ts
export const ConfigSchema = z.object({ auto_add_download: z.boolean() });
export type Config = z.infer<typeof ConfigSchema>;
```

### 4.9 Add small helpers

- `export const DEFAULT_REQUEST_HEADERS: RequestHeaders = { cookies: null, referrer: null, user_agent: null };`
- Keep `GetStatusFullResultSchema` / `DownloadStatusMapSchema` (both are `Record<string, DownloadStatus>`).

### 4.10 Unchanged / verified-correct

`DownloadStateSchema`, `DownloadStatusSchema`, `SegmentInfoSchema`, `HashAlgoSchema`, `VerifierSchema`,
`VerifierMaybeSchema`, `DownloadDataSchema`, `DownloadDataMapSchema`, `DownloadInfoSchema` all match the
Rust wire format exactly (verified in §2).

---

## 5. Plan — `src/code/apiCommunicate.ts`

| Function | Change |
|----------|--------|
| `addDownload(url, filepath, numberOfSegments, verifier, requestHeaders?)` | **Add `requestHeaders` arg** and send it as `requestHeaders` (Tauri maps camelCase → `request_headers`). Default to `DEFAULT_REQUEST_HEADERS` when omitted. Parse errors with `ManagerApiErrorSchema`. |
| `openFile()` | Parse with `z.string().nullable()` → returns `string \| null` (G8). |
| `getStatus()` | Unchanged shape; parse with `DownloadStatusMapSchema`; error mapping → `ManagerApiErrorSchema`. |
| `getItemStatus(id)` | **New.** `invoke("get_item_status", { id })` → `DownloadStatus`. |
| `getDownloadMetadata(url)` | Unchanged; error mapping → `DownloadInfoGetApiErrorSchema`. |
| `pause(id)` / `resume(id)` | Add try/catch → `ManagerApiErrorSchema` (G9). Return `Promise<void>`. |
| `getData()` | Unchanged shape (schema fixed in §4.7); error mapping → `ManagerApiErrorSchema`. |
| `getConfigFile()` | **New.** `invoke("get_config_file")` → `Config`. |
| `saveSession()` | **New.** `invoke("save_session")` → `void`; error mapping → `DataSerializationErrorApiErrorSchema`. |
| `getFilenameFromPath()` | **Delete** — no such backend command (G7). `utils.ts#getFilenameFromPath` is the one in use. |

Implementation conventions for the file:
- Every command that can fail gets a `try { ... } catch (err) { throw <ApiErrorSchema>.parse(err) }`.
- Keep the existing `console.error(err)` before rethrowing where it is already present.
- Use a single shared `invoke` import (already there) and keep the explicit `key: value` object style
  already used in the file.

---

## 6. Call-site updates required for the app to keep compiling/behaving

These are outside the two target files but are forced by the plan:

1. `src/code/useDownloads.ts`
   - `useAddDownload` mutation payload gains `requestHeaders?: RequestHeaders` and forwards it to
     `addDownload(...)`.
   - `useDownloads.ts` `BaseAppError` stays compatible (`message`, `kind?`, `payload?`) — the new
     `*ApiError` types satisfy it.
   - Optional: add `useConfigFile()` / `useSaveSession()` hooks built on the new wrappers.
2. `src/components/adddownload/runDownload.tsx`
   - `addDownload.mutate({...})` can pass `requestHeaders` (e.g. from a user-agent/referrer input in the
     future); with the default parameter no change is strictly required.
3. `src/components/adddownload/addDownloadUrl.tsx`
   - `DownloadInfoGetErrorSchema.parse(err)` → `DownloadInfoGetApiErrorSchema.parse(err)` so `errParsed.message`
     is typed; `err?.payload` prop type becomes `DownloadInfoGetApiError`.
4. `src/components/downloadtable/downloadtable.tsx` — pause/resume `err.message` usage keeps working once
   the wrappers normalise errors to `{ message, kind, payload }`.

---

## 7. Implementation order

1. `typesOfBackend.ts`: add `RequestHeadersSchema`, `ConfigSchema`, `DataSerializationErrorSchema`,
   `withApiMessage` helper; fix `DownloadRunErrorSchema`, `ManagerErrorSchema`,
   `DownloadInfoGetErrorSchema`, `InPlaceSegmentDataSchema`.
2. `apiCommunicate.ts`: rewrite the wrappers per §5 (add `requestHeaders`, add the 3 missing commands,
   delete the dead `getFilenameFromPath`, fix `openFile` nullability, add error mapping to pause/resume).
3. Update the call sites in §6.
4. Verify (below).

---

## 8. Verification

- `cd /storage/Programming/dashload-reloaded && npx tsc --noEmit` (or the repo's typecheck script) — zero errors.
- Run the app (`npm run tauri dev`):
  - Add a download → `add_download` must succeed (proves `requestHeaders` is sent) and the row must appear
    in the table (proves `get_data` parses, i.e. `headers` field fix).
  - Pause/resume a download → buttons work, no raw Tauri error objects in the console.
  - Force a failing download (bad URL) → the status parses (`Failed` state) and the badge shows red.
- Optional sanity check: temporarily `console.log` the raw `invoke("get_data")` payload and compare with
  the zod schema in §4.7.

---

## 9. Out of scope

- `src-tauri/src/events.rs` HTTP-server feature (`external-download-request` event) — separate API surface.
- Backend changes (e.g. adding `#[serde(default)]` to `RequestHeaders`, or renaming `user_agent`) —
  this plan adapts the frontend to the backend as it exists today.
