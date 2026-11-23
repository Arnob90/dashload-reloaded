import { invoke } from "@tauri-apps/api/core";
import {
    Config,
    ConfigSchema,
} from "./typesOfBackend";

/**
 * Fetches and validates the backend config file.
 * Returns the config data without storing it in mutable globals.
 */
export async function getConfigFile(): Promise<Config> {
    const res = await invoke("get_config_file");
    return await ConfigSchema.parseAsync(res);
}

/**
 * Query key for the config file.
 * Used with TanStack Query to cache and refetch the config on mutations.
 */
export const configKey = ["config"] as const;

/**
 * Retrieves the config file using TanStack Query.
 * This should be used via useQuery instead of accessing mutable globals.
 */
export async function fetchConfigFile(): Promise<Config> {
    return await getConfigFile();
}

export type { Config } from "./typesOfBackend";