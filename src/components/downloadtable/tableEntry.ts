import {
    useDownloadData,
    useDownloadStatus,
} from "../../code/useDownloads.ts";
import { DownloadState, FailureDetails } from "../../code/typesOfBackend.ts";
import { useEffect, useMemo } from "react";
import { notifications } from "@mantine/notifications";
import { getFilenameFromPath, calculateEta } from "../../code/utils.ts";

export interface TableEntry {
    id: string;
    filename: string;
    url: string;
    downloaded: number;
    total: number | null;
    speed: number | null;
    eta: number | null;
    status: DownloadState;
    downloadedPortion: number | null;
    filepath: string;
    failureDetails: FailureDetails | null;
    headers: {
        userAgent: string | null;
        referrer: string | null;
        cookies: string | null;
    };
}

/**
 * Builds the `TableEntry[]` array from the latest status + data snapshots.
 * Returns `null` only when there is *no* cached data yet (first load); on
 * transient query errors the last known-good entries are returned so the
 * download table never unmounts mid-flight.
 */
function buildEntries(
    statusDatas: Record<string, any> | null | undefined,
    downloadDatas: Record<string, any> | null | undefined,
): TableEntry[] | null {
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

        const segmentHeaders = data.segments[0]?.headers;

        // Extract rich failure metadata when the state is Failed.
        const failureDetails: FailureDetails | null =
            status.state.kind === "Failed"
                ? status.state.payload
                : null;

        res.push({
            id,
            url: data.url,
            filename: getFilenameFromPath(data.filepath),
            downloaded: status.downloaded,
            total: status.total,
            speed: calculatedSpeed,
            eta: calculateEta(status.total, status.downloaded, calculatedSpeed),
            status: status.state,
            downloadedPortion: portion,
            filepath: data.filepath,
            failureDetails,
            headers: {
                userAgent: segmentHeaders?.user_agent ?? null,
                referrer: segmentHeaders?.referrer ?? null,
                cookies: segmentHeaders?.cookies ?? null,
            },
        });
    }
    res.sort((a, b) => a.id.localeCompare(b.id));
    return res;
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

    const entries = useMemo(
        () => buildEntries(statusDatas, downloadDatas),
        [statusDatas, downloadDatas],
    );

    // Only block the UI on the *first* load — when we have never yet fetched
    // any data. On transient errors, keep the last known-good entries alive
    // so the download table never unmounts mid-flight.
    if (isPendingStatus && isPendingDownload && entries === null) {
        return null;
    }

    return entries;
}