import { invoke } from "@tauri-apps/api/core";
import {
    SegmentItemDownloadItemCreationErrorSchema,
    NormalDownloadItemCreationErrorSchema,
    PauseErrorSchema,
    ResumeErrorSchema,
} from "./typesOfBackend";
export async function startDownload(url: string, filepath: string) {
    try {
        await invoke("start_download_normal", {
            givenUrl: url,
            filepath: filepath,
        });
    }
    catch (err) {
        const err_maybe = await NormalDownloadItemCreationErrorSchema.safeParseAsync(err);
        if (err_maybe.success) {
            return err_maybe.data
        }
        console.error(err)
        throw err
    }

}
export async function startDownloadSegmented(
    url: string,
    filepath: string,
    numberOfSegments: number
) {
    try {
        await invoke("start_download_segmented", {
            givenUrl: url,
            filepath: filepath,
            numberOfSegments: numberOfSegments,
        });
    }
    catch (err) {
        const err_maybe = await SegmentItemDownloadItemCreationErrorSchema.safeParseAsync(err);
        if (err_maybe.success) {
            return err_maybe.data
        }
        console.error(err)
        throw err
    }
}

export async function pause(id: string) {
    try {
        await invoke("pause", { id: id });
    }
    catch (err) {
        const err_maybe = await PauseErrorSchema.safeParseAsync(err);
        if (err_maybe.success) {
            return err_maybe.data
        }
        console.error(err)
        throw err
    }
}

export async function resume(id: string) {
    try {
        await invoke("resume", { id: id });
    }
    catch (err) {
        const err_maybe = await ResumeErrorSchema.safeParseAsync(err);
        if (err_maybe.success) {
            return err_maybe.data
        }
        console.error(err)
        throw err
    }
}
