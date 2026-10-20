import { invoke } from "@tauri-apps/api/core"
import z from "zod"
import { DownloadDataMap, DownloadDataMapSchema, DownloadInfo, DownloadInfoSchema, DownloadStatusMapSchema, DownloadStatusMap, ManagerApiErrorSchema, DownloadInfoGetApiErrorSchema, VerifierMaybe, DEFAULT_REQUEST_HEADERS, ConfigSchema, Config, DataSerializationApiErrorSchema, DownloadStatusSchema } from "./typesOfBackend"
import type { DownloadStatus } from "./typesOfBackend"
export async function addDownload(
    url: string,
    filepath: string,
    numberOfSegments: number,
    verifier: VerifierMaybe,
    requestHeaders?: typeof DEFAULT_REQUEST_HEADERS
): Promise<string> {
    try {
        const idMaybe = await invoke("add_download", {
            url: url,
            filepath: filepath,
            numberOfSegments: numberOfSegments,
            verifier: verifier,
            requestHeaders: requestHeaders ?? DEFAULT_REQUEST_HEADERS
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

export async function getDownloadMetadata(url: string): Promise<DownloadInfo> {
    try {
        const infoMaybe = await invoke("get_download_metadata", {
            url: url
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
    try {
        await invoke("pause", {
            id: id
        })
    }
    catch (err) {
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

export async function getConfigFile(): Promise<Config> {
    try {
        const res = await invoke("get_config_file")
        return await ConfigSchema.parseAsync(res)
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

// Legacy function for backward compatibility (not used by the UI)
export async function getFilenameFromPath(filepath: string): Promise<string | null> {
    const returned = await invoke("get_filename_from_path", {
        filepath: filepath
    })
    const parsed = await z.string().nullable().parseAsync(returned)
    return parsed
}
