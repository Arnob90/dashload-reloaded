import { invoke } from "@tauri-apps/api/core";
import {
    SegmentItemDownloadItemCreationErrorSchema,
    PauseErrorSchema,
    ResumeErrorSchema,
    SegmentItemDownloadItemCreationError,
    PauseError,
    ResumeError,
    GetStatusFullResult,
} from "./typesOfBackend";
import { useMutation, useQueryClient } from "@tanstack/react-query";
async function startDownloadSegmented(
    url: string,
    filepath: string,
    numberOfSegments: number
): Promise<SegmentItemDownloadItemCreationError | undefined> {
    try {
        await invoke("start_download_segmented", {
            givenUrl: url,
            filepath: filepath,
            numberOfSegments: numberOfSegments,
        });
    }
    catch (err) {
        const err_maybe = await SegmentItemDownloadItemCreationErrorSchema.safeParseAsync(err);
        if (err_maybe.success) {
            return err_maybe.data
        }
        console.error(err)
        throw err
    }
}

export function useStartDownloadSegmented() {
    const queryClient = useQueryClient();
    return useMutation({
        mutationFn: async (args: { url: string; filepath: string; numberOfSegments: number }) => {
            //No need for a on error, the function throws the unexpected stuff, and returns the error(which is expected :))
            return startDownloadSegmented(args.url, args.filepath, args.numberOfSegments)
        },
        onMutate: async (args: { url: string; filepath: string; numberOfSegments: number }) => {
            await queryClient.cancelQueries({ queryKey: ["get_status"] });
            const previous = queryClient.getQueryData<GetStatusFullResult>(["get_status"])
            const randomCacheId = crypto.randomUUID();
            queryClient.setQueryData(["get_status"], {
                ...previous,
                [randomCacheId]: {
                    filepath: args.filepath,
                    filename: args.filepath.split("/").pop() ?? "",
                    url: args.url,
                    status: {
                        kind: "Running",
                        payload: {
                            downloaded: 0,
                            downloaded_portion: 0,
                            download_speed: 0,
                            eta_secs: 0,
                            total: 0
                        }
                    }
                },
            } satisfies GetStatusFullResult);
            return { previous };
        },
        onSettled: (_data, _error, _variables, _context) => {
            queryClient.invalidateQueries({ queryKey: ["get_status"] })
        },
        onError: (_data, _variables, context) => {
            queryClient.setQueryData(["get_status"], context?.previous)
        }
    })
}



async function pause(id: string): Promise<PauseError | undefined> {
    try {
        await invoke("pause", { id: id });
    }
    catch (err) {
        const err_maybe = await PauseErrorSchema.safeParseAsync(err);
        if (err_maybe.success) {
            return err_maybe.data
        }
        console.error(err)
        throw err
    }
}

export function usePause() {
    const queryClient = useQueryClient()
    return useMutation({
        mutationFn: async (args: { id: string }) => {
            return pause(args.id)
        },
        onMutate: async (args: { id: string }) => {
            await queryClient.cancelQueries({ queryKey: ["get_status"] });
            const previous = queryClient.getQueryData<GetStatusFullResult>(["get_status"])
            if (previous === undefined) {
                return { previous }
            }
            const updated = { ...previous, [args.id]: { ...previous[args.id], status: { kind: "Paused" } } } satisfies GetStatusFullResult;
            queryClient.setQueryData<GetStatusFullResult>(["get_status"], updated);
            return { previous };
        },
        onSettled: (_data, _error, _variables, _context) => {
            queryClient.invalidateQueries({ queryKey: ["get_status"] })
        },
        onError: (_data, _variables, context) => {
            queryClient.setQueryData(["get_status"], context?.previous)
        },
    })
}

async function resume(id: string): Promise<ResumeError | undefined> {
    try {
        await invoke("resume", { id: id });
    }
    catch (err) {
        const err_maybe = await ResumeErrorSchema.safeParseAsync(err);
        if (err_maybe.success) {
            return err_maybe.data
        }
        console.error(err)
        throw err
    }
}
export function useResume() {
    const queryClient = useQueryClient()
    return useMutation({
        mutationFn: async (args: { id: string }) => {
            return resume(args.id)
        },
        onMutate: async (args: { id: string }) => {
            await queryClient.cancelQueries({ queryKey: ["get_status"] });
            const previous = queryClient.getQueryData<GetStatusFullResult>(["get_status"])
            if (previous === undefined) {
                return { previous }
            }
            const updated = {
                ...previous, [args.id]: {
                    ...previous[args.id], status: {
                        kind: "Running", payload: {
                            downloaded: 0,
                            total: 0,
                            downloaded_portion: null,
                            eta_secs: null,
                            download_speed: 0

                        }
                    }
                }
            } satisfies GetStatusFullResult;
            queryClient.setQueryData<GetStatusFullResult>(["get_status"], updated);
            return { previous };
        },
        onSettled: (_data, _error, _variables, _context) => {
            queryClient.invalidateQueries({ queryKey: ["get_status"] })
        },
        onError: (_data, _variables, context) => {
            queryClient.setQueryData(["get_status"], context?.previous)
        },
    })
}
