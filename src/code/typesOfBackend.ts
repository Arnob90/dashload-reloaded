import { z } from "zod";

// ============================================================================
// 0. Shared API wrapper helper
// ============================================================================
/** Rust: ApiError<T> { message: String, #[serde(flatten)] error: T } */
export function withApiMessage<T extends z.ZodTypeAny>(errorSchema: T) {
    return z.intersection(z.object({ message: z.string() }), errorSchema);
}
export type ApiError<T extends z.ZodTypeAny> = z.infer<ReturnType<typeof withApiMessage<T>>>;

// ============================================================================
// 1. Request headers
// ============================================================================
export const RequestHeadersSchema = z.object({
    cookies: z.string().nullable(),
    referrer: z.string().nullable(),
    user_agent: z.string().nullable(),
});
export type RequestHeaders = z.infer<typeof RequestHeadersSchema>;

export const DEFAULT_REQUEST_HEADERS: RequestHeaders = { cookies: null, referrer: null, user_agent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:128.0) Gecko/20100101 Firefox/128.0", };

// ============================================================================
// 2. Config
// ============================================================================
export const ConfigSchema = z.object({ auto_add_download: z.boolean() });
export type Config = z.infer<typeof ConfigSchema>;

// ============================================================================
// 3. Core Execution Errors (DownloadRunError)
// ============================================================================

export const DownloadRunErrorSchema = z.discriminatedUnion("kind", [
    z.object({
        kind: z.literal("NetworkError"),
        payload: z.string(),
    }),
    z.object({
        kind: z.literal("ServerError"),
        payload: z.string(),
    }),
    z.object({
        kind: z.literal("DiskError"),
        payload: z.string(),
    }),
    z.object({
        kind: z.literal("AlreadyRunning"),
    }),
    z.object({
        kind: z.literal("VerifyError"),
    }),
]);

export type DownloadRunError = z.infer<typeof DownloadRunErrorSchema>;

// ============================================================================
// 2. Download State & Status
// ============================================================================

export const DownloadStateSchema = z.discriminatedUnion("kind", [
    z.object({ kind: z.literal("Stopped") }),
    z.object({
        kind: z.literal("Running"),
        payload: z.object({
            download_speed: z.number(), // Speed in bytes per second
        }),
    }),
    z.object({
        kind: z.literal("Completed"),
        payload: z.object({
            size: z.number(),
        }),
    }),
    z.object({
        kind: z.literal("Failed"),
        payload: DownloadRunErrorSchema,
    }),
    z.object({ kind: z.literal("Paused") }),
    z.object({ kind: z.literal("Verifying") }),
]);

export type DownloadState = z.infer<typeof DownloadStateSchema>;

/** Struct + Enum DownloadStatus */
export const DownloadStatusSchema = z.object({
    total: z.number().nullable(),
    downloaded: z.number(),
    state: DownloadStateSchema,
});

export type DownloadStatus = z.infer<typeof DownloadStatusSchema>;

export const DownloadStatusMapSchema = z.record(z.string(), DownloadStatusSchema)
export type DownloadStatusMap = z.infer<typeof DownloadStatusMapSchema>

/** Returned by get_status command: HashMap<String, DownloadStatus> */
export const GetStatusFullResultSchema = z.record(
    z.string(),
    DownloadStatusSchema
);

export type GetStatusFullResult = z.infer<typeof GetStatusFullResultSchema>;

// ============================================================================
// 3. Data serialization errors
// ============================================================================
export const DataSerializationErrorSchema = z.discriminatedUnion("kind", [
    z.object({
        kind: z.literal("IoError"),
        payload: z.string(),
    }),
    z.object({
        kind: z.literal("DeserializationError"),
        payload: z.string(),
    }),
]);

export type DataSerializationError = z.infer<typeof DataSerializationErrorSchema>;

// ============================================================================
// 4. Manager & Orchestration Errors (ManagerError)
// Replaces SegmentItemDownloadItemCreationError, PauseError, ResumeError, etc.
// ============================================================================

export const ManagerErrorSchema = z.discriminatedUnion("kind", [
    z.object({
        kind: z.literal("IdDoesNotExist"),
    }),
    z.object({
        kind: z.literal("NoFilesizeError"),
    }),
    z.object({
        kind: z.literal("DownloadCreationError"),
        payload: z.string(),
    }),
    z.object({
        kind: z.literal("OperationFailed"),
        payload: z.string(),
    }),
]);

export type ManagerError = z.infer<typeof ManagerErrorSchema>;
export const ManagerApiErrorSchema = withApiMessage(ManagerErrorSchema);
export type ManagerApiError = z.infer<typeof ManagerApiErrorSchema>;

// ============================================================================
// 4. Download Metadata & Info
// ============================================================================

export const DownloadInfoSchema = z.object({
    url: z.string(),
    filename: z.string().nullable(),
    filesize: z.number().nullable(),
    segmented_possible: z.boolean(),
});

export type DownloadInfo = z.infer<typeof DownloadInfoSchema>;

export const DownloadInfoGetErrorSchema = z.discriminatedUnion("kind", [
    z.object({
        kind: z.literal("RequestError"),
        payload: z.string(),
    }),
    z.object({
        kind: z.literal("UrlError"),
        payload: z.string(),
    }),
]);

export type DownloadInfoGetError = z.infer<typeof DownloadInfoGetErrorSchema>;
export const DownloadInfoGetApiErrorSchema = withApiMessage(DownloadInfoGetErrorSchema);
export type DownloadInfoGetApiError = z.infer<typeof DownloadInfoGetApiErrorSchema>;

export const SegmentInfoSchema = z.object({
    start: z.number(),
    end: z.number()
})

export type SegmentInfo = z.infer<typeof SegmentInfoSchema>



export const InPlaceSegmentDataSchema = z.object({
    url: z.string(),
    filepath: z.string(),
    last_downloaded: z.number(),
    info: SegmentInfoSchema.nullable(),
    headers: RequestHeadersSchema,
})

export type InPlaceSegmentData = z.infer<typeof InPlaceSegmentDataSchema>

export const HashAlgoSchema = z.enum(["Sha256", "Sha512", "Md5"]);
export type HashAlgo = z.infer<typeof HashAlgoSchema>;

export const VerifierSchema = z.object({
    to_match_hash: z.string(),
    algo: HashAlgoSchema,
});
export type Verifier = z.infer<typeof VerifierSchema>;

export const VerifierMaybeSchema = z.object({
    verifier: VerifierSchema.nullable(),
});
export type VerifierMaybe = z.infer<typeof VerifierMaybeSchema>;

export const DownloadDataSchema = z.object({
    url: z.string(),
    filepath: z.string(),
    segments: z.array(InPlaceSegmentDataSchema),
    length: z.number().nullable(),
    state: DownloadStateSchema,
    verifier: VerifierMaybeSchema

})
export type DownloadData = z.infer<typeof DownloadDataSchema>

export const DownloadDataMapSchema = z.record(z.string(), DownloadDataSchema)
export type DownloadDataMap = z.infer<typeof DownloadDataMapSchema>

// Export new schemas that are used in apiCommunicate.ts
export const DataSerializationApiErrorSchema = withApiMessage(DataSerializationErrorSchema);
export type DataSerializationApiError = z.infer<typeof DataSerializationApiErrorSchema>;

