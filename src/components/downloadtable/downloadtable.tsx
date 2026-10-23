import { useCallback, useMemo, useState } from "react";
import { ActionIcon, Table, Text, Menu, Group, Badge, Progress } from "@mantine/core";
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
} from "../../code/useDownloads.ts";
import { notifications } from "@mantine/notifications";
import { ChevronRight, FolderOpen, Pause, Trash, FileArchive, FileCode, File as DefaultFile } from "lucide-react";
import { revealItemInDir, openPath } from "@tauri-apps/plugin-opener"
import { useTableEntries, TableEntry } from "./tableEntry.ts";
import { formatTime } from "../../code/utils.ts";

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
    const { mutate: pauseDownload } = usePauseDownload();
    const { mutate: resumeDownload } = useResumeDownload();
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
                size: 110,
                cell: (info) => {
                    const kind = info.getValue().kind
                    const colorMap: Record<string, string> = {
                        Running: "blue",
                        Completed: "teal",
                        Paused: "yellow",
                        Failed: "red",
                        Stopped: "gray"
                    }
                    return <Badge color={colorMap[kind]} variant="light" size="sm">
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

                    return (
                        <Group gap="xs" wrap="nowrap" justify="center" mx="auto" style={{ width: "100%", maxHeight: 220 }}>
                            <Progress
                                value={pct}
                                size="md" // 👈 8px thickness instead of 4px
                                radius="xl" // 👈 Smooth pill corners
                                color={pct === 100 ? "teal" : "blue"}
                                striped={pct < 100 && pct > 0}
                                animated={pct < 100 && pct > 0}
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
                            <Menu.Item onClick={() => handleOpenFile(row.original.filepath)}>
                                Open File
                            </Menu.Item>
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
