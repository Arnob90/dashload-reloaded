import { invoke } from "@tauri-apps/api/core";

export async function startDownload(url:string, filepath:string) {
    await invoke("start_download_normal",{givenUrl:url,filepath:filepath})
}
