import { z } from "zod";

/** Inner enum: DownloadRunError */
export const DownloadRunErrorSchema = z.discriminatedUnion("kind", [
    z.object({ kind: z.literal("RequestError"), payload: z.string(), message: z.string() }),
    z.object({ kind: z.literal("LengthlessServerError"), message: z.string() }),
    z.object({ kind: z.literal("RangeError"), message: z.string() }),
    z.object({ kind: z.literal("IllBehavedServer"), message: z.string() }),
    z.object({ kind: z.literal("ServerNotRangable"), message: z.string() }),
    z.object({ kind: z.literal("InvalidHeaderValue"), message: z.string() }),
    z.object({ kind: z.literal("FileOpenError"), message: z.string() }),
    z.object({ kind: z.literal("ServerNotAvailable"), message: z.string() }),
    z.object({ kind: z.literal("FileSeekError"), message: z.string() }),
    z.object({ kind: z.literal("FileWriteError"), payload: z.string(), message: z.string() }),
    z.object({ kind: z.literal("AlreadyRunning"), message: z.string() }),
]);

export type DownloadRunError = z.infer<typeof DownloadRunErrorSchema>;

export function get_error_message_from_run_error(error: DownloadRunError): string {
    return error.message;
}

export const DownloadStatusSchema = z.discriminatedUnion("kind", [
    z.object({ kind: z.literal("Stopped") }),
    z.object({
        kind: z.literal("Running"),
        payload: z.object({
            total: z.number().nullable(),
            downloaded: z.number(),
            download_speed: z.number(),
            downloaded_portion: z.number().nullable(),
            eta_secs: z.number().nullable(),
        }),
    }),
    z.object({ kind: z.literal("Completed") }),
    z.object({
        kind: z.literal("Failed"),
        payload: DownloadRunErrorSchema,
    }),
    z.object({
        kind: z.literal("Paused"),
    }),
]);

export const IDownloadDataSchema = z.object({
    filepath: z.string(),
    filename: z.string(),
    url: z.string(),
    status: DownloadStatusSchema,
});
export type IDownloadData = z.infer<typeof IDownloadDataSchema>;
export type DownloadStatus = z.infer<typeof DownloadStatusSchema>;

export const GetStatusFullResultSchema = z.record(
    z.string(),
    IDownloadDataSchema
);
export type GetStatusFullResult = z.infer<typeof GetStatusFullResultSchema>;

export const SegmentItemDownloadItemCreationErrorSchema = z.discriminatedUnion(
    "kind",
    [
        z.object({ kind: z.literal("RequestError"), payload: z.string(), message: z.string() }),
        z.object({ kind: z.literal("SegmentationLengthError"), payload: z.string(), message: z.string() }),
        z.object({ kind: z.literal("SegmentFailed"), payload: z.string(), message: z.string() }),
        z.object({ kind: z.literal("NonUnicodePath"), payload: z.string(), message: z.string() }),
        z.object({ kind: z.literal("LengthGettingError"), payload: z.string(), message: z.string() }),
        z.object({ kind: z.literal("SerializationError"), payload: z.string(), message: z.string() }),
        z.object({ kind: z.literal("InvalidFilepathError"), payload: z.string(), message: z.string() })
    ]
);
export type SegmentItemDownloadItemCreationError = z.infer<typeof SegmentItemDownloadItemCreationErrorSchema>;


export const DownloadStatusGetErrorSchema = z.discriminatedUnion("kind", [
    z.object({ kind: z.literal("StatusCheckError"), payload: z.string(), message: z.string() }),
    z.object({ kind: z.literal("IdDoesNotExist"), payload: z.string(), message: z.string() })
]);
export type DownloadStatusGetError = z.infer<typeof DownloadStatusGetErrorSchema>;


export const DownloadInfoSchema = z.object({
    url: z.string(),
    filename: z.string().nullable(),
    filesize: z.number().nullable(),
    segmented_possible: z.boolean()
});
export type DownloadInfo = z.infer<typeof DownloadInfoSchema>;

export const DownloadInfoGetErrorSchema = z.discriminatedUnion("kind", [
    z.object({ kind: z.literal("RequestError"), payload: z.string(), message: z.string() }),
    z.object({ kind: z.literal("UrlError"), payload: z.string(), message: z.string() })
]);
export type DownloadInfoGetError = z.infer<typeof DownloadInfoGetErrorSchema>;

export const DownloadInfoGetResultSchema = z.union([DownloadInfoSchema, DownloadInfoGetErrorSchema]);
export type DownloadInfoGetResult = z.infer<typeof DownloadInfoGetResultSchema>;

export const NormalDownloadItemCreationErrorSchema = z.discriminatedUnion("kind", [
    z.object({ kind: z.literal("DirectoryGivenError"), payload: z.string(), message: z.string() })
]);
export type NormalDownloadItemCreationError = z.infer<typeof NormalDownloadItemCreationErrorSchema>;


export const ResumeErrorSchema = z.discriminatedUnion("kind", [
    z.object({ kind: z.literal("IdDoesNotExist"), payload: z.string(), message: z.string() }),
    z.object({ kind: z.literal("AlreadyRunning"), payload: z.string(), message: z.string() }),
    z.object({ kind: z.literal("NormalDownloadCreationError"), payload: NormalDownloadItemCreationErrorSchema, message: z.string() }),
    z.object({ kind: z.literal("SegmentedDownloadItemCreationError"), payload: SegmentItemDownloadItemCreationErrorSchema, message: z.string() }),
    z.object({ kind: z.literal("MiscError"), payload: z.string(), message: z.string() })
]);
export type ResumeError = z.infer<typeof ResumeErrorSchema>;


export const PauseErrorSchema = z.discriminatedUnion("kind", [
    z.object({ kind: z.literal("IdDoesNotExist"), payload: z.string(), message: z.string() }),
    z.object({ kind: z.literal("NotRunningError"), payload: z.string(), message: z.string() }),
]);
export type PauseError = z.infer<typeof PauseErrorSchema>;
