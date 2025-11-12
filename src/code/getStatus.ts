import { invoke } from "@tauri-apps/api/core";
import {
    GetStatusFullResult,
    GetStatusFullResultSchema,
} from "./typesOfBackend";
import { useQuery } from "@tanstack/react-query";
export function useDownloadStatuses() {
    return useQuery<GetStatusFullResult>({
        queryKey: ["get_status"],
        queryFn: async () => {
            const requiredStatusMaybe = await invoke("get_status");
            const parsedData =
                await GetStatusFullResultSchema.parseAsync(requiredStatusMaybe);
            return parsedData;
        },
        //Default 300
        // TODO: Make this configurable in the future
        refetchInterval: 300,
        refetchIntervalInBackground: false,
    });
}
