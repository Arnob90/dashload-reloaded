import { invoke } from "@tauri-apps/api/core"
import z from "zod"
import { DownloadDataMap, DownloadDataMapSchema, DownloadInfo, DownloadInfoSchema, DownloadStatusMapSchema, DownloadStatusMap, ManagerErrorSchema, DownloadInfoGetErrorSchema, VerifierMaybe } from "./typesOfBackend"
export async function addDownload(url: string, filepath: string, numberOfSegments: number, verifier: VerifierMaybe): Promise<string> {
    try {
        const idMaybe = await invoke("add_download", {
            url: url,
            filepath: filepath,
            numberOfSegments: numberOfSegments,
            verifier: verifier
        })
        const id = await z.string().parseAsync(idMaybe)
        return id
    }
    catch (err) {
        throw ManagerErrorSchema.parse(err)
    }
}

export async function openFile(): Promise<string | null> {
    const pickedMaybe = await invoke("open_file")
    const picked = await z.string().parseAsync(pickedMaybe)
    return picked
}

export async function getStatus(): Promise<DownloadStatusMap> {
    try {
        const statusMaybe = await invoke("get_status")
        return await DownloadStatusMapSchema.parseAsync(statusMaybe)
    }
    catch (err) {
        console.error(err)
        throw ManagerErrorSchema.parse(err)
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
        throw DownloadInfoGetErrorSchema.parse(err)
    }
}


export async function pause(id: string) {
    await invoke("pause", {
        id: id
    })
}
export async function resume(id: string) {
    await invoke("resume", {
        id: id
    })
}
export async function getData(): Promise<DownloadDataMap> {
    const res = await invoke("get_data")
    const parsed = await DownloadDataMapSchema.parseAsync(res)
    return parsed
}

export async function getFilenameFromPath(filepath: string) {
    const returned = await invoke("get_filename_from_path", {
        filepath: filepath
    })
    const parsed = await z.string().nullable().parseAsync(returned)
    return parsed
}
