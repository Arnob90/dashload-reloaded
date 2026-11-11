import { useCallback, useMemo, useState } from "react";
import { ActionIcon, Box, Flex, Group, Badge, Menu, Progress, Table, Text, Tooltip } from "@mantine/core";
import { bytesToMIB } from "../../code/utils.ts";
import {
    createColumnHelper,
    useTable,
    createSortedRowModel,
    rowSortingFeature,
    flexRender,
    SortingState,
    tableFeatures,
    columnSizingFeature
} from "@tanstack/react-table";
import style from "./downloadtable.module.css";
import { SelectableItemsTable } from "../selectabletable/selectableItemsTable.tsx";
import {
    usePauseDownload,
    useResumeDownload,
    useDeleteDownload,
    useRetryDownload,
} from "../../code/useDownloads.ts";
import { notifications } from "@mantine/notifications";
import { ChevronRight, FolderOpen, Pause, Trash, FileArchive, FileCode, File as DefaultFile, RotateCcw, CircleAlert } from "lucide-react";
import { revealItemInDir, openPath } from "@tauri-apps/plugin-opener"
import { useTableEntries, TableEntry } from "./tableEntry.ts";
import { formatTime } from "../../code/utils.ts";
import { DownloadDetailsPane } from "./downloadDetails.tsx";
import type { FailureDetails } from "../../code/typesOfBackend.ts";

/** Short, human-readable summary of a FailureDetails payload for tooltips. */
function summarizeFailureDetails(details: FailureDetails): string {
    const { error } = details;
    let summary: string;
    switch (error.kind) {
        case "VerifyError":
            summary = `Checksum mismatch (expected ${error.payload.expected}, got ${error.payload.actual})`;
            break;
        case "TimeoutError":
            summary = `Timed out after ${error.payload.secs}s`;
            break;
        case "AlreadyRunning":
            summary = "Download is already running";
            break;
        default:
            summary = error.payload.message;
    }
    const attempts = details.attempts_made > 0 ? ` · ${details.attempts_made} attempt(s) made` : "";
    return `${summary}${attempts}`;
}

const features = tableFeatures({
    rowSortingFeature,
    columnSizingFeature,
    sortedRowModel: createSortedRowModel(),
});

function getFileIcon(filename: string) {
    if (filename.endsWith(".iso") || filename.endsWith(".tar.gz") || filename.endsWith(".zip")) {
        return <FileArchive size={16} color="#4dabf7" />;
    }
    if (filename.endsWith(".exe") || filename.endsWith(".deb") || filename.endsWith(".rpm")) {
        return <FileCode size={16} color="#ff922b" />;
    }
    return <DefaultFile size={16} color="#868e96" />;
}

const columnHelper = createColumnHelper<typeof features, TableEntry>();
export function DownloadTable() {
    const entries = useTableEntries();
    const tableData = useMemo(() => entries ?? [], [entries]);
    const [selectedId, setSelectedId] = useState<string | null>(null);
    const selectedEntry = entries?.find((e) => e.id === selectedId);
    const { mutate: pauseDownload } = usePauseDownload();
    const { mutate: resumeDownload } = useResumeDownload();
    const { mutate: deleteDownload } = useDeleteDownload();
    const { mutate: retryDownload } = useRetryDownload();
    const [sorting, setSorting] = useState<SortingState>([])
    const handleOpenInFileManager = (path: string) => {
        revealItemInDir(path)
    }
    const handleOpenFile = (path: string) => {
        openPath(path)
    }
    const handlePause = useCallback((id: string) => {
        console.log("Attempt pause for id:", id);
        pauseDownload(id, {
            onError: (err) => {
                notifications.show({
                    title: "Failed to Pause",
                    message: err.message,
                    color: "red",
                });
            },
        });
    }, [pauseDownload]);

    const handleResume = useCallback((id: string) => {
        resumeDownload(id, {
            onError: (err) => {
                notifications.show({
                    title: "Failed to Resume",
                    message: err.message,
                    color: "red",
                });
            },
        });
    }, [resumeDownload]);

    const handleRetry = useCallback((id: string) => {
        retryDownload(id, {
            onError: (err) => {
                notifications.show({
                    title: "Failed to Retry",
                    message: err.message,
                    color: "red",
                });
            },
            onSuccess: () => {
                notifications.show({
                    title: "Retrying",
                    message: "The download has been restarted.",
                    color: "blue",
                });
            },
        });
    }, [retryDownload]);
    const handleDelete = useCallback((id: string) => {
        deleteDownload(id, {
            onError: (err) => {
                notifications.show({
                    title: "Failed to Delete",
                    message: err.message,
                    color: "red",
                });
            },
            onSuccess: () => {
                setSelectedId(null);
            },
        });
    }, [deleteDownload]);
    const columns = useMemo(
        () => columnHelper.columns([
            columnHelper.display({
                id: "actions",
                header: "",
                size: 50,
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
                                onMouseDown={(e) => e.stopPropagation()}
                            >
                                <ChevronRight size={18} style={{ pointerEvents: "none" }} />
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
                                <Pause size={18} style={{ pointerEvents: "none" }} />
                            </ActionIcon>
                        );
                    }

                    if (stateKind === "Failed" || stateKind === "Stopped") {
                        return (
                            <ActionIcon
                                variant="subtle"
                                color="red"
                                onClick={(e) => {
                                    e.stopPropagation(); // Prevents row selection toggle on button click
                                    handleRetry(info.row.original.id);
                                }}
                                onMouseDown={(e) => e.stopPropagation()}
                            >
                                <RotateCcw size={18} style={{ pointerEvents: "none" }} />
                            </ActionIcon>
                        );
                    }

                    return null;
                },
            }),
            columnHelper.accessor("filename", {
                header: "Filename",
                cell: (info) => (
                    <Group gap="xs" wrap="nowrap">
                        {getFileIcon(info.getValue())}
                        <Text size="sm" fw={500} truncate maw={300}>
                            {info.getValue()}
                        </Text>
                    </Group>
                )
            }),
            columnHelper.accessor("status", {
                header: "Status",
                size: 130,
                cell: (info) => {
                    const state = info.getValue();
                    const kind = state.kind
                    const colorMap: Record<string, string> = {
                        Running: "blue",
                        Completed: "teal",
                        Paused: "yellow",
                        Failed: "red",
                        Stopped: "gray",
                        Verifying: "violet",
                        Retrying: "orange",
                    }
                    const color = colorMap[kind] ?? "gray";

                    if (kind === "Retrying") {
                        const { attempt, max_attempts } = state.payload;
                        return <Badge color={color} variant="light" size="sm" leftSection={<RotateCcw size={12} />}>
                            Retrying ({attempt}/{max_attempts})
                        </Badge>;
                    }

                    if (kind === "Failed") {
                        const label = (
                            <Group gap="xs" wrap="nowrap">
                                <Badge color={color} variant="light" size="sm" style={{ pointerEvents: "none" }}>
                                    Failed
                                </Badge>
                                <CircleAlert size={14} color="var(--mantine-color-red-6)" style={{ pointerEvents: "none" }} />
                            </Group>
                        );
                        return (
                            <Tooltip
                                label={info.row.original.failureDetails ? summarizeFailureDetails(info.row.original.failureDetails) : "Download failed"}
                                w={300}
                                multiline
                                withArrow
                            >
                                {label}
                            </Tooltip>
                        );
                    }

                    return <Badge color={color} variant="light" size="sm">
                        {kind}
                    </Badge>
                },
            }),
            columnHelper.accessor("downloaded", {
                header: "Downloaded",
                size: 100,
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
                size: 100,
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
                size: 110,
                cell: (info) => (
                    <Text>
                        {info.getValue() != null
                            ? `${bytesToMIB(info.getValue()!).toFixed(2)} MiB/s`
                            : "-"}
                    </Text>
                ),
            }),
            // In column definitions:
            columnHelper.accessor("downloadedPortion", {
                size: 180,
                header: () => (
                    <Text size="xs" fw={700} c={"dimmed"} tt="uppercase" ta={"center"}>
                        Progress
                    </Text>
                ),
                cell: (info) => {
                    const val = info.getValue();
                    if (val === null) return <Text size="xs" c="dimmed">-</Text>;
                    const pct = Math.round(val * 100);
                    const kind = info.row.original.status.kind;
                    const color = kind === "Failed" ? "red" : pct === 100 ? "teal" : "blue";

                    return (
                        <Group gap="xs" wrap="nowrap" justify="center" mx="auto" style={{ width: "100%", maxHeight: 220 }}>
                            <Progress
                                value={pct}
                                size="md" // 👈 8px thickness instead of 4px
                                radius="xl" // 👈 Smooth pill corners
                                color={color}
                                striped={pct < 100 && pct > 0 && kind !== "Failed"}
                                animated={pct < 100 && pct > 0 && kind !== "Failed"}
                                style={{ flex: 1 }} // 👈 Automatically fills the available column space
                            />
                            <Text size="xs" fw={600} w={38} ta="right" c={pct === 100 ? "teal" : undefined}>
                                {pct}%
                            </Text>
                        </Group>
                    );
                },
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
        [handlePause, handleResume, handleRetry]
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
        <Flex direction="column" h="100%" flex="1" style={{ overflow: "hidden" }}>
            {/* 1. Main Table (Takes remaining flex height) */}
            <Box style={{ flex: 1, overflowY: "auto" }}>
                <SelectableItemsTable setSelectedId={setSelectedId} verticalSpacing="sm" horizontalSpacing="md" highlightOnHover style={{ tableLayout: "auto" }}>
                    <Table.Thead bg="dark.8">
                        {tableModel.getHeaderGroups().map((headerGroup) => (
                            <Table.Tr key={headerGroup.id}>
                                {headerGroup.headers.map((header) => (
                                    <Table.Th key={header.id}>
                                        <Text size="xs" fw={700} c="dimmed" tt="uppercase">
                                            {header.isPlaceholder
                                                ? null
                                                : flexRender(
                                                    header.column.columnDef.header,
                                                    header.getContext()
                                                )}
                                        </Text>
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
                                    {(row.original.status.kind === "Failed" || row.original.status.kind === "Stopped") && (
                                        <Menu.Item leftSection={<RotateCcw size={16} />} onClick={() => handleRetry(row.original.id)}>
                                            Retry Download
                                        </Menu.Item>
                                    )}
                                    <Menu.Item onClick={() => handleOpenFile(row.original.filepath)}>
                                        Open File
                                    </Menu.Item>
                                    <Menu.Item leftSection={<FolderOpen size={16} />}
                                        onClick={() => handleOpenInFileManager(row.original.filepath)}
                                    >
                                        Open in File Manager
                                    </Menu.Item>
                                    <Menu.Item leftSection={<Trash size={16} />} className={style.delete_menu_item} onClick={() => handleDelete(row.original.id)}>
                                        Delete
                                    </Menu.Item>
                                </Menu.Dropdown>
                            </Menu>
                        ))}
                    </Table.Tbody>
                </SelectableItemsTable>
            </Box>

            {/* 2. FDM-Style Bottom Details Pane */}
            {selectedEntry && <DownloadDetailsPane entry={selectedEntry} onClose={() => setSelectedId(null)} />}
        </Flex>
    );
}
