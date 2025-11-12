import { invoke } from "@tauri-apps/api/core";
import {
    SegmentItemDownloadItemCreationErrorSchema,
    NormalDownloadItemCreationErrorSchema,
    SmartDownloadItemCreationErrorSchema
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
export async function startDownloadSmart(
    url: string,
    filepath: string
) {
    try {
        await invoke("start_download_smart", {
            givenUrl: url,
            filepath: filepath,
        });
    }
    catch (err) {
        const err_maybe = await SmartDownloadItemCreationErrorSchema.safeParseAsync(err);
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
