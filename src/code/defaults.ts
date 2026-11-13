import { invoke } from "@tauri-apps/api/core";
import {
    Config,
    ConfigSchema,
    RequestHeaders,
    RetryScheme,
} from "./typesOfBackend";

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

/** Mirrors `RetryScheme::default()`; overridden by the config file. */
const FALLBACK_RETRY_SCHEME: RetryScheme = {
    max_attempts: 5,
    backoff: {
        base_delay: { secs: 1, nanos: 0 },
        max_delay: { secs: 30, nanos: 0 },
        factor: 2.0,
        jitter: true,
    },
    stream_timeout: { secs: 5, nanos: 0 },
    connection_timeout: { secs: 10, nanos: 0 },
};

/** Config loaded from the backend at startup by `initConfigFile()`. */
export let CONFIG: Config | null = null;

/**
 * Default request headers used by download commands and the UI.
 * Replaced with the config file's `request_headers` during initialization.
 */
export let DEFAULT_REQUEST_HEADERS: RequestHeaders = FALLBACK_REQUEST_HEADERS;

/**
 * Retry policy applied to new downloads.
 * Replaced with the config file's `retry_scheme` during initialization.
 */
export let DEFAULT_RETRY_SCHEME: RetryScheme = FALLBACK_RETRY_SCHEME;

/**
 * Default download path used by the download form.
 * Replaced with the config file's `default_download_path` during initialization.
 */
export let DEFAULT_DOWNLOAD_PATH: string = "";

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
    DEFAULT_RETRY_SCHEME = config.retry_scheme;
    DEFAULT_DOWNLOAD_PATH = config.default_download_path;
    return config;
}

