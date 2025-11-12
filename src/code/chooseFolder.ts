import { invoke } from "@tauri-apps/api/core";
import z from "zod";
const StringMaybeSchema = z.string().nullable()

export async function chooseFolder(): Promise<string | null> {
    try {
        const choosen = await invoke("open_file")
        const parsed = await StringMaybeSchema.parseAsync(choosen)
        return parsed
    }
    catch (err) {
        console.error(err)
        throw err
    }
}
