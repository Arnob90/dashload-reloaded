import { invoke } from "@tauri-apps/api/core";
import { DownloadInfoGetResultSchema, DownloadInfoGetErrorSchema } from "./typesOfBackend";

export async function getDownloadInfo(url: string) {
    try {
        const returned = await invoke("get_download_metadata", { url: url });
        const parsed = await DownloadInfoGetResultSchema.parseAsync(returned);
        return parsed
    }
    catch (err) {
        const err_maybe = DownloadInfoGetErrorSchema.safeParse(err);
        if (err_maybe.success) {
            return err_maybe.data
        }
        console.error(err)
        throw err
    }
}
