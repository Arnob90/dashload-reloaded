import { useQuery } from "@tanstack/react-query";
import { configQueryKey } from "./useDownloads";
import { fetchConfigFile } from "./defaults";
import type { Config } from "./typesOfBackend";

/**
 * Hook that provides the current config from the backend.
 * Automatically refetches when the Tauri state broadcasts a new config.
 */
export function useAppConfig(): Config {
    const { data } = useQuery<Config>({
        queryKey: configQueryKey,
        queryFn: fetchConfigFile,
        // staleTime is set low so we get fresh data on component mount/render
        staleTime: 1000,
    });

    return data ?? {
        auto_add_download: false,
        default_download_path: "",
        default_number_of_segments: 1,
        retry_scheme: {
            max_attempts: 5,
            backoff: {
                base_delay: { secs: 1, nanos: 0 },
                max_delay: { secs: 30, nanos: 0 },
                factor: 2.0,
                jitter: true,
            },
            stream_timeout: { secs: 5, nanos: 0 },
            connection_timeout: { secs: 10, nanos: 0 },
        },
        request_headers: {
            cookies: null,
            referrer: null,
            user_agent:
                "Mozilla/5.0 (Windows NT 10.0; rv:128.0) Gecko/20100101 Firefox/128.0",
        },
        polling_rate: "300000",
    };
}