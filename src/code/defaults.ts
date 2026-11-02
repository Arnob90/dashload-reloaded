import { invoke } from "@tauri-apps/api/core";
import { Config, ConfigSchema, RequestHeaders } from "./typesOfBackend";

/**
 * Fallback used only until `initConfigFile()` runs (or if it fails).
 * The real value comes from the backend config file's `request_headers`.
 */
const FALLBACK_REQUEST_HEADERS: RequestHeaders = {
    cookies: null,
    referrer: null,
    user_agent:
        "Mozilla/5.0 (Windows NT 10.0; rv:128.0) Gecko/20100101 Firefox/128.0",
};

/** Config loaded from the backend at startup by `initConfigFile()`. */
export let CONFIG: Config | null = null;

/**
 * Default request headers used by download commands and the UI.
 * Replaced with the config file's `request_headers` during initialization.
 */
export let DEFAULT_REQUEST_HEADERS: RequestHeaders = FALLBACK_REQUEST_HEADERS;

/** Fetches and validates the backend config file. */
export async function getConfigFile(): Promise<Config> {
    const res = await invoke("get_config_file");
    return await ConfigSchema.parseAsync(res);
}

/**
 * Loads the backend config once at startup and initializes the frontend
 * defaults that are derived from it.
 */
export async function initConfigFile(): Promise<Config> {
    const config = await getConfigFile();
    CONFIG = config;
    DEFAULT_REQUEST_HEADERS = config.request_headers;
    return config;
}
