import {
    useDownloadData,
    useDownloadStatus,
} from "../../code/useDownloads.ts";
import { DownloadState } from "../../code/typesOfBackend.ts";
import { useEffect, useMemo } from "react";
import { notifications } from "@mantine/notifications";
import { getFilenameFromPath, calculateEta } from "../../code/utils.ts";

export interface TableEntry {
    id: string;
    filename: string;
    downloaded: number;
    total: number | null;
    speed: number | null;
    eta: number | null;
    status: DownloadState;
    downloadedPortion: number | null;
    filepath: string
}

export function useTableEntries(): TableEntry[] | null {
    const {
        data: statusDatas,
        isPending: isPendingStatus,
        error: statusGettingError,
    } = useDownloadStatus();

    const {
        data: downloadDatas,
        isPending: isPendingDownload,
        error: downloadDataGettingError,
    } = useDownloadData();

    useEffect(() => {
        if (statusGettingError != null) {
            notifications.show({
                title: "Failed to get status",
                message: statusGettingError.message,
                color: "red",
            });
            console.error(statusGettingError)
        }
        if (downloadDataGettingError != null) {
            notifications.show({
                title: "Failed to fetch download data",
                message: downloadDataGettingError.message,
                color: "red",
            });
        }
    }, [statusGettingError, downloadDataGettingError]);
    const entries = useMemo(() => {
        if (!statusDatas || !downloadDatas) return null;

        const res: TableEntry[] = [];

        for (const id of Object.keys(statusDatas)) {
            const data = downloadDatas[id];
            const status = statusDatas[id];

            if (!data || !status) continue;

            const calculatedSpeed =
                status.state.kind === "Running"
                    ? status.state.payload.download_speed
                    : null;

            const portion =
                status.total && status.total > 0
                    ? status.downloaded / status.total
                    : null;

            res.push({
                id,
                filename: getFilenameFromPath(data.filepath),
                downloaded: status.downloaded,
                total: status.total,
                speed: calculatedSpeed,
                eta: calculateEta(status.total, status.downloaded, calculatedSpeed),
                status: status.state,
                downloadedPortion: portion,
                filepath: data.filepath
            });
        }
        res.sort((a, b) => a.id.localeCompare(b.id));
        return res;
    }, [statusDatas, downloadDatas]);

    if (
        isPendingDownload ||
        isPendingStatus ||
        statusGettingError != null ||
        downloadDataGettingError != null
    ) {
        return null;
    }

    return entries;
}
