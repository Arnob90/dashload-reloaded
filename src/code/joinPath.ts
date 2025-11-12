import { invoke } from "@tauri-apps/api/core"
import z from "zod"

const stringSchema = z.string()

export async function joinPaths(path1: string, path2: string) {
    try {
        const pathMaybe = await invoke<string>("join_paths", { firstSegment: path1, secondSegment: path2 })
        const parsed = await stringSchema.parseAsync(pathMaybe)
        return parsed
    }
    catch (err) {
        console.error(err)
        throw err
    }
}
