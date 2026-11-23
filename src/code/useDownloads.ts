import {
    useQuery,
    useMutation,
    useQueryClient,
} from "@tanstack/react-query";
import {
    addDownload,
    openFile,
    getStatus,
    getDownloadMetadata,
    pause,
    resume,
    retryDownload,
    getData,
    getItemStatus,
    saveSession,
    deleteDownload,
    stopDownload,
    setConfigFile,
} from "./apiCommunicate.ts";
import type { DownloadStatusMap, DownloadInfo, DownloadDataMap, RequestHeaders, DownloadOptions, Config } from "./typesOfBackend";

// ============================================================================
// 1. Centralized Query Keys
// ============================================================================
export const downloadKeys = {
    all: ["downloads"] as const,
    status: () => [...downloadKeys.all, "status"] as const,
    data: () => [...downloadKeys.all, "data"] as const,
    metadata: (url: string) => [...downloadKeys.all, "metadata", url] as const,
};

export const configQueryKey = ["config"] as const;

// ============================================================================
// 2. Query Hooks (Data Fetching & Polling)
// ============================================================================

/**
 * Retrieves the backend config file using TanStack Query.
 * This query will automatically refetch when set_config_file is called on the backend.
 */
export function useConfig() {
    return useQuery<Config>({
        queryKey: configQueryKey,
        queryFn: () => import("./defaults").then(m => m.fetchConfigFile()),
        staleTime: 1000 * 60 * 5, // 5 minutes
    });
}

/**
 * Fetches the current config as part of downloads data.
 * This is separate from the main config query so it can be used independently.
 */
export function useDownloadConfig() {
    return useQuery<Config>({
        queryKey: configQueryKey,
        queryFn: () => import("./defaults").then(m => m.fetchConfigFile()),
    });
}

/**
 * Polls active download statuses every `pollIntervalMs` (default 300ms).
 * Uses `placeholderData` to prevent UI flicker between polls.
 */
export function useDownloadStatus(pollIntervalMs = 300) {
    return useQuery<DownloadStatusMap>({
        queryKey: downloadKeys.status(),
        queryFn: getStatus,
        refetchInterval: pollIntervalMs,
        placeholderData: (previousData) => previousData,
    });
}

/**
 * Fetches metadata (filename, filesize, range support) for a given URL.
 * Only executes when a non-empty URL is provided.
 */
export function useDownloadMetadata(url: string | undefined, requestHeaders?: RequestHeaders) {
    return useQuery<DownloadInfo>({
        queryKey: downloadKeys.metadata(url ?? ""),
        queryFn: () => {
            if (!url?.trim()) {
                throw new Error("A non empty URL is required to fetch metadata.");
            }
            return getDownloadMetadata(url, requestHeaders);
        },
        enabled: Boolean(url?.trim()),
        staleTime: 1000 * 60 * 5, // Cache metadata for 5 minutes
    });
}

/** Metadata mutation variables interface */
export interface MetadataMutationVariables {
    url: string;
    requestHeaders?: RequestHeaders;
}

/**
 * Fetches metadata via a mutation hook.
 */
export function useFetchMetadata() {
    return useMutation({
        mutationFn: ({ url, requestHeaders }: MetadataMutationVariables) => getDownloadMetadata(url, requestHeaders),
    });
}

/**
 * Polls full persistent snapshot data (for session exports or debugging).
 */
export function useDownloadData(pollIntervalMs = 300) {
    return useQuery<DownloadDataMap>({
        queryKey: downloadKeys.data(),
        queryFn: getData,
        refetchInterval: pollIntervalMs,
        placeholderData: (prevData) => prevData,
    });
}

// ============================================================================
// 3. Mutation Hooks (Actions)
// ============================================================================

/**
 * Adds a new download and immediately invalidates the status query cache.
 * Also refetches the config to pick up any changes.
 */
export function useAddDownload() {
    const queryClient = useQueryClient();

    return useMutation({
        mutationFn: (opts: DownloadOptions) => addDownload(opts),
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: downloadKeys.all });
            queryClient.invalidateQueries({ queryKey: configQueryKey });
        },
    });
}

/**
 * Pauses a download by ID and immediately invalidates the status query cache.
 */
export function usePauseDownload() {
    const queryClient = useQueryClient();

    return useMutation({
        mutationFn: (id: string) => pause(id),
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: downloadKeys.status() });
        },
    });
}

/**
 * Resumes a download by ID and immediately invalidates the status query cache.
 */
export function useResumeDownload() {
    const queryClient = useQueryClient();

    return useMutation({
        mutationFn: (id: string) => resume(id),
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: downloadKeys.status() });
        },
    });
}

/**
 * Retries a failed or stopped download by ID and invalidates the status cache.
 */
export function useRetryDownload() {
    const queryClient = useQueryClient();

    return useMutation({
        mutationFn: (id: string) => retryDownload(id),
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: downloadKeys.all });
        },
    });
}

/**
 * Opens native file/folder picker dialog.
 */
export function useOpenFile() {
    return useMutation({
        mutationFn: openFile,
    });
}

/** Gets the status of a specific download item */
export function useGetItemStatus() {
    return useMutation({
        mutationFn: getItemStatus,
    });
}

/** Gets the config file */
export function useGetConfigFile() {
    return useMutation({
        mutationFn: () => import("./defaults").then(m => m.fetchConfigFile()),
    });
}

/** Saves the current session */
export function useSaveSession() {
    return useMutation({
        mutationFn: saveSession,
    });
}

/**
 * Updates the backend config file and broadcasts changes to the frontend.
 * On success, refetches config via TanStack Query to keep UI in sync.
 */
export function useSetConfig() {
    const queryClient = useQueryClient();

    return useMutation({
        mutationFn: (config: Config) => setConfigFile(config),
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: configQueryKey });
            queryClient.invalidateQueries({ queryKey: downloadKeys.all });
        },
    });
}

/**
 * Deletes a download by ID (stops it, removes it from the manager, and
 * invalidates the status query cache).
 */
export function useDeleteDownload() {
    const queryClient = useQueryClient();

    return useMutation({
        mutationFn: (id: string) => deleteDownload(id),
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: downloadKeys.all });
        },
    });
}


/**
 * Stops a download by ID (stops it, and
 * invalidates the status query cache).
 */
export function useStopDownload() {
    const queryClient = useQueryClient();

    return useMutation({
        mutationFn: (id: string) => stopDownload(id),
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: downloadKeys.all });
        },
    });
}

// Base error guaranteed across ALL Tauri commands
export interface BaseAppError {
    message: string;
    kind?: string;
    payload?: unknown;
}

// Global TanStack Query Module Augmentation
declare module "@tanstack/react-query" {
    interface Register {
        defaultError: BaseAppError; // Guarantees error.message exists on ALL queries/mutations
    }
}