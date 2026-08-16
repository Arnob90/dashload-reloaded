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
    getData,
} from "./apiCommunicate.ts"; // Your invoke file
import type { DownloadStatusMap, DownloadInfo, DownloadDataMap } from "./typesOfBackend";

// ============================================================================
// 1. Centralized Query Keys
// ============================================================================
export const downloadKeys = {
    all: ["downloads"] as const,
    status: () => [...downloadKeys.all, "status"] as const,
    data: () => [...downloadKeys.all, "data"] as const,
    metadata: (url: string) => [...downloadKeys.all, "metadata", url] as const,
};

// ============================================================================
// 2. Query Hooks (Data Fetching & Polling)
// ============================================================================

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
export function useDownloadMetadata(url: string) {
    return useQuery<DownloadInfo>({
        queryKey: downloadKeys.metadata(url),
        queryFn: () => getDownloadMetadata(url),
        enabled: Boolean(url && url.trim().length > 0),
        staleTime: 1000 * 60 * 5, // Cache metadata for 5 minutes
    });
}

export function useFetchMetadata() {
    return useMutation({
        mutationFn: getDownloadMetadata,
    });
}

/**
 * Fetches full persistent snapshot data (for session exports or debugging).
 */
export function useDownloadData(pollIntervalMs = 300) {
    return useQuery<DownloadDataMap>({
        queryKey: downloadKeys.data(),
        queryFn: getData,
        refetchInterval: pollIntervalMs,
        placeholderData: (prevData) => prevData

    });
}

// ============================================================================
// 3. Mutation Hooks (Actions)
// ============================================================================

/**
 * Adds a new download and immediately invalidates the status query cache.
 */
export function useAddDownload() {
    const queryClient = useQueryClient();

    return useMutation({
        mutationFn: ({
            url,
            filepath,
            numberOfSegments,
        }: {
            url: string;
            filepath: string;
            numberOfSegments: number;
        }) => addDownload(url, filepath, numberOfSegments),
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: downloadKeys.all })
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
 * Opens native file/folder picker dialog.
 */
export function useOpenFile() {
    return useMutation({
        mutationFn: openFile,
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
