import { useCallback, useEffect, useMemo, useState } from "react";
import { ActionIcon, Table, Text, Menu } from "@mantine/core";
import { bytesToMIB } from "../../code/utils.ts";
import {
    createColumnHelper,
    useTable,
    createSortedRowModel,
    rowSortingFeature,
    flexRender,
    SortingState,
    tableFeatures
} from "@tanstack/react-table";
import style from "./downloadtable.module.css";
import { SelectableItemsTable } from "../selectabletable/selectableItemsTable.tsx";
import {
    useDownloadData,
    useDownloadStatus,
    usePauseDownload,
    useResumeDownload,
} from "../../code/useDownloads.ts";
import { notifications } from "@mantine/notifications";
import { ChevronRight, FolderOpen, Pause, Trash } from "lucide-react";
import { revealItemInDir } from "@tauri-apps/plugin-opener"
import type { DownloadState } from "../../code/typesOfBackend.ts";

function formatTime(seconds: number): string {
    if (seconds <= 0 || !isFinite(seconds)) return "-";

    const hrs = Math.floor(seconds / 3600);
    const mins = Math.floor((seconds % 3600) / 60);
    const secs = Math.floor(seconds % 60);

    const paddedHrs = String(hrs).padStart(2, "0");
    const paddedMins = String(mins).padStart(2, "0");
    const paddedSecs = String(secs).padStart(2, "0");

    return `${paddedHrs}:${paddedMins}:${paddedSecs}`;
}

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

export function getFilenameFromPath(filepath: string): string {
    return filepath.trim().replace(/[/\\]+$/, "").split(/[/\\]/).pop() || "_";
}

export function calculateEta(
    total: number | null,
    downloaded: number,
    speed: number | null
): number | null {
    if (total === null || speed === null || speed <= 0) {
        return null;
    }
    const remaining = Math.max(0, total - downloaded);
    return remaining / speed;
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
            console.log(statusGettingError)
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
// ✅ Move OUTSIDE DownloadTable (at the top of the file)
const features = tableFeatures({
    rowSortingFeature,
    sortedRowModel: createSortedRowModel(),
});

const columnHelper = createColumnHelper<typeof features, TableEntry>();

export function DownloadTable() {
    const entries = useTableEntries();
    const tableData = useMemo(() => entries ?? [], [entries]);
    const [selectedId, setSelectedId] = useState<string | null>(null);
    const pauseMutation = usePauseDownload();
    const resumeMutation = useResumeDownload();
    const [sorting, setSorting] = useState<SortingState>([])
    const handleOpenInFileManager = (path: string) => {
        revealItemInDir(path)
    }
    const handlePause = useCallback((id: string) => {
        pauseMutation.mutate(id, {
            onError: (err) => {
                notifications.show({
                    title: "Failed to Pause",
                    message: err.message,
                    color: "red",
                });
            },
        });
    }, [pauseMutation]);

    const handleResume = useCallback((id: string) => {
        resumeMutation.mutate(id, {
            onError: (err) => {
                notifications.show({
                    title: "Failed to Resume",
                    message: err.message,
                    color: "red",
                });
            },
        });
    }, [resumeMutation]);
    const columns = useMemo(
        () => columnHelper.columns([
            columnHelper.display({
                id: "actions",
                header: "",
                cell: (info) => {
                    const stateKind = info.row.original.status.kind;

                    if (stateKind === "Paused") {
                        return (
                            <ActionIcon
                                variant="subtle"
                                onClick={(e) => {
                                    e.stopPropagation(); // Prevents row selection toggle on button click
                                    handleResume(info.row.original.id);
                                }}
                            >
                                <ChevronRight size={18} />
                            </ActionIcon>
                        );
                    }

                    if (stateKind === "Running") {
                        return (
                            <ActionIcon
                                variant="subtle"
                                onClick={(e) => {
                                    e.stopPropagation(); // Prevents row selection toggle on button click
                                    handlePause(info.row.original.id);
                                }}
                            >
                                <Pause size={18} />
                            </ActionIcon>
                        );
                    }

                    return null;
                },
            }),
            columnHelper.accessor("filename", {
                header: "Filename",
                cell: (info) => info.getValue(),
            }),
            columnHelper.accessor("status", {
                header: "Status",
                cell: (info) => info.getValue().kind,
            }),
            columnHelper.accessor("downloaded", {
                header: "Downloaded",
                cell: (info) => (
                    <Text>
                        {info.getValue() != null
                            ? `${bytesToMIB(info.getValue()).toFixed(1)} MiB`
                            : "-"}
                    </Text>
                ),
            }),
            columnHelper.accessor("total", {
                header: "Total",
                cell: (info) => (
                    <Text>
                        {info.getValue() != null
                            ? `${bytesToMIB(info.getValue()!).toFixed(1)} MiB`
                            : "-"}
                    </Text>
                ),
            }),
            columnHelper.accessor("speed", {
                header: "Speed",
                cell: (info) => (
                    <Text>
                        {info.getValue() != null
                            ? `${bytesToMIB(info.getValue()!).toFixed(2)} MiB/s`
                            : "-"}
                    </Text>
                ),
            }),
            columnHelper.accessor("downloadedPortion", {
                header: "Progress",
                cell: (info) => (
                    <Text>
                        {info.getValue() != null
                            ? `${(info.getValue()! * 100).toFixed(1)}%`
                            : "-"}
                    </Text>
                ),
            }),
            columnHelper.accessor("eta", {
                header: "ETA",
                cell: (info) => (
                    <Text>
                        {info.getValue() != null ? formatTime(info.getValue()!) : "-"}
                    </Text>
                ),
            }),
        ]),
        [handlePause, handleResume]
    );

    const tableModel = useTable({
        features,
        data: tableData,
        columns,
        getRowId: (row) => row.id,
        onSortingChange: setSorting,
        state: {
            sorting
        }
    });

    if (entries === null) {
        return <Text p="md">Loading downloads...</Text>;
    }

    return (
        <SelectableItemsTable setSelectedId={setSelectedId}>
            <Table.Thead>
                {tableModel.getHeaderGroups().map((headerGroup) => (
                    <Table.Tr key={headerGroup.id}>
                        {headerGroup.headers.map((header) => (
                            <Table.Th key={header.id}>
                                {header.isPlaceholder
                                    ? null
                                    : flexRender(
                                        header.column.columnDef.header,
                                        header.getContext()
                                    )}
                            </Table.Th>
                        ))}
                    </Table.Tr>
                ))}
            </Table.Thead>
            <Table.Tbody>
                {tableModel.getRowModel().rows.map((row) => (
                    <Menu key={row.id} shadow="md" width={200} withinPortal>
                        <Menu.ContextMenu>
                            <Table.Tr
                                key={row.id}
                                tabIndex={0}
                                onClick={() => {
                                    if (selectedId === row.original.id) {
                                        setSelectedId(null);
                                        return;
                                    }
                                    setSelectedId(row.original.id);
                                }}
                                className={
                                    row.original.id === selectedId
                                        ? `${style.selected_row}`
                                        : `${style.table_row}`
                                }
                            >
                                {row.getAllCells().map((cell) => (
                                    <Table.Td key={cell.id}>
                                        {flexRender(cell.column.columnDef.cell, cell.getContext())}
                                    </Table.Td>
                                ))}
                            </Table.Tr>
                        </Menu.ContextMenu>
                        <Menu.Dropdown>
                            <Menu.Item leftSection={<FolderOpen size={16} />}
                                onClick={() => handleOpenInFileManager(row.original.filepath)}
                            >
                                Open in File Manager
                            </Menu.Item>
                            <Menu.Item leftSection={<Trash size={16} />} className={style.delete_menu_item}>
                                Delete
                            </Menu.Item>
                        </Menu.Dropdown>
                    </Menu>
                ))}
            </Table.Tbody>
        </SelectableItemsTable>
    );
}
