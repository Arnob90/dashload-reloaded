import { z } from "zod";

/** Inner enum: DownloadRunError */
export const DownloadRunErrorSchema = z.discriminatedUnion("kind", [
    z.object({ kind: z.literal("RequestError"), payload: z.string() }),
    z.object({ kind: z.literal("LengthlessServerError") }),
    z.object({ kind: z.literal("RangeError") }),
    z.object({ kind: z.literal("IllBehavedServer") }),
    z.object({ kind: z.literal("ServerNotRangable") }),
    z.object({ kind: z.literal("InvalidHeaderValue") }),
    z.object({ kind: z.literal("FileOpenError") }),
    z.object({ kind: z.literal("ServerNotAvailable") }),
    z.object({ kind: z.literal("FileSeekError") }),
    z.object({ kind: z.literal("FileWriteError"), payload: z.string() }),
    z.object({ kind: z.literal("AlreadyRunning") }),
]);

/** Outer enum: DownloadStatusCheckError */
export const DownloadStatusCheckErrorSchema = z.discriminatedUnion("kind", [
    z.object({ kind: z.literal("DroppedDownloadError") }),
    z.object({ kind: z.literal("DownloadRunError"), payload: DownloadRunErrorSchema }),
]);



/** TypeScript types inferred automatically */
export type DownloadRunError = z.infer<typeof DownloadRunErrorSchema>;
export type DownloadStatusCheckError = z.infer<typeof DownloadStatusCheckErrorSchema>;


export function get_error_message_from_run_error(error: DownloadRunError) {
    switch (error.kind) {
        case "RequestError":
            return `Request failed! Reason: ${error.payload}`;
        case "LengthlessServerError":
            return "The server does not have a concept of filesize.";
        case "RangeError":
            return "The requested range is out of bounds.";
        case "IllBehavedServer":
            return "The server violates basic protocols.";
        case "ServerNotRangable":
            return "The server does not support range requests.";
        case "InvalidHeaderValue":
            return "Received an invalid header value from the server.";
        case "FileOpenError":
            return "Failed to open the file for writing.";
        case "ServerNotAvailable":
            return "The server is currently unavailable.";
        case "FileSeekError":
            return "Failed to seek within the file.";
        case "FileWriteError":
            return `Cannot write to file: ${error.payload}`;
        case "AlreadyRunning":
            return "The download cannot be run twice.";
        default:
            return "Unknown download run error.";
    }
}


export function get_error_message_from_status_error(error: DownloadStatusCheckError): string {
    switch (error.kind) {
        case "DroppedDownloadError":
            return "The download was dropped and cannot be checked.";

        case "DownloadRunError": {
            const inner = error.payload;
            return get_error_message_from_run_error(inner)
        }

        default:
            return "Unknown download status check error.";
    }
}

export const DownloadStatusSchema = z.discriminatedUnion("kind", [
    z.object({ kind: z.literal("Pending") }),
    z.object({ kind: z.literal("Stopped") }),
    z.object({
        kind: z.literal("Running"),
        payload: z.object({
            total: z.number(),
            downloaded: z.number()
        })
    }),
    z.object({ kind: z.literal("Completed") }),
    z.object({
        kind: z.literal("Failed"),
        payload: DownloadRunErrorSchema
    })
])
export type DownloadStatus = z.infer<typeof DownloadStatusSchema>

export const GetStatusFullResultSchema = z.record(z.string(), DownloadStatusSchema)
export type GetStatusFullResult = z.infer<typeof GetStatusFullResultSchema>
