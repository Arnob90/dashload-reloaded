import { invoke } from "@tauri-apps/api/core"
import z from "zod"
import { DownloadDataMap, DownloadDataMapSchema, DownloadInfo, DownloadInfoSchema, DownloadStatusMapSchema, DownloadStatusMap, ManagerApiErrorSchema, DownloadInfoGetApiErrorSchema, RequestHeaders, DataSerializationApiErrorSchema, DownloadStatusSchema, DownloadOptions, DownloadOptionsSchema } from "./typesOfBackend"
import type { DownloadStatus } from "./typesOfBackend"
import { DEFAULT_REQUEST_HEADERS } from "./defaults"
export async function addDownload(opts: DownloadOptions): Promise<string> {
    try {
        const parsed = await DownloadOptionsSchema.parseAsync(opts)
        const idMaybe = await invoke("add_download", {
            opts: parsed,
        })
        const id = await z.string().parseAsync(idMaybe)
        return id
    }
    catch (err) {
        throw ManagerApiErrorSchema.parse(err)
    }
}

export async function openFile(): Promise<string | null> {
    const pickedMaybe = await invoke("open_file")
    const picked = await z.string().nullable().parseAsync(pickedMaybe)
    return picked
}

export async function getStatus(): Promise<DownloadStatusMap> {
    try {
        const statusMaybe = await invoke("get_status")
        return await DownloadStatusMapSchema.parseAsync(statusMaybe)
    }
    catch (err) {
        console.error(err)
        throw ManagerApiErrorSchema.parse(err)
    }
}

export async function getDownloadMetadata(url: string, requestHeaders?: RequestHeaders): Promise<DownloadInfo> {
    try {
        const infoMaybe = await invoke("get_download_metadata", {
            url: url,
            requestHeaders: requestHeaders ?? DEFAULT_REQUEST_HEADERS
        })
        const info = await DownloadInfoSchema.parseAsync(infoMaybe)
        return info
    }
    catch (err) {
        console.error(err)
        throw DownloadInfoGetApiErrorSchema.parse(err)
    }
}


export async function pause(id: string): Promise<void> {
    console.log("Pausing download with id:", id)
    try {
        await invoke("pause", {
            id: id
        })
        console.log("Paused download with id:", id)
    }
    catch (err) {
        console.error("Error pausing download with id:", id, err)
        throw ManagerApiErrorSchema.parse(err)
    }
}
export async function resume(id: string): Promise<void> {
    try {
        await invoke("resume", {
            id: id
        })
    }
    catch (err) {
        throw ManagerApiErrorSchema.parse(err)
    }
}

export async function retryDownload(id: string): Promise<string> {
    try {
        const idMaybe = await invoke("retry_download", {
            id: id,
        })
        return await z.string().parseAsync(idMaybe)
    }
    catch (err) {
        throw ManagerApiErrorSchema.parse(err)
    }
}
export async function getData(): Promise<DownloadDataMap> {
    try {
        const res = await invoke("get_data")
        const parsed = await DownloadDataMapSchema.parseAsync(res)
        return parsed
    }
    catch (err) {
        throw ManagerApiErrorSchema.parse(err)
    }
}

export async function getItemStatus(id: string): Promise<DownloadStatus> {
    try {
        const res = await invoke("get_item_status", {
            id: id
        })
        return await DownloadStatusSchema.parseAsync(res)
    }
    catch (err) {
        throw ManagerApiErrorSchema.parse(err)
    }
}

export async function saveSession(): Promise<void> {
    try {
        await invoke("save_session")
    }
    catch (err) {
        throw DataSerializationApiErrorSchema.parse(err)
    }
}

export async function deleteDownload(id: string): Promise<void> {
    try {
        await invoke("delete_download", {
            id: id,
        })
    }
    catch (err) {
        throw ManagerApiErrorSchema.parse(err)
    }
}

export async function stopDownload(id: string): Promise<void> {
    try {
        await invoke("stop_download", {
            id: id,
        })
    }
    catch (err) {
        throw ManagerApiErrorSchema.parse(err)
    }

}
