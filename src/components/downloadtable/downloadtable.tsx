import { useMemo } from "react"
import { GetStatusFullResult } from "../../code/typesofbackend"
import { List } from "immutable"
import { Table, Text } from "@mantine/core"
import { createColumnHelper, getCoreRowModel, useReactTable, flexRender } from "@tanstack/react-table"
export type StatusFullResultValue = GetStatusFullResult[keyof GetStatusFullResult]
export interface StatusView {
    filename: string,
    status: string,
    downloaded: number | null,
    total: number | null,
    downloadSpeed: number | null,
    downloaded_portion: number | null,
    eta_secs: number | null
}

function bytesToMIB(bytes: number): number {
    return Number((bytes / 1024 / 1024).toFixed(2))
}
function formatTime(seconds: number): string {
    const hrs = Math.floor(seconds / 3600);
    const mins = Math.floor((seconds % 3600) / 60);
    const secs = Math.floor(seconds % 60);

    // Pad with leading zeros if needed
    const paddedHrs = String(hrs).padStart(2, '0');
    const paddedMins = String(mins).padStart(2, '0');
    const paddedSecs = String(secs).padStart(2, '0');

    return `${paddedHrs}:${paddedMins}:${paddedSecs}`;
}
function convertStatusToView(givenStatus: StatusFullResultValue): StatusView {
    const filename = givenStatus.filename;
    const status = givenStatus.status.kind;
    if (status === "Running") {
        return {
            filename,
            status,
            downloadSpeed: givenStatus.status.payload.download_speed,
            downloaded: givenStatus.status.payload.downloaded,
            total: givenStatus.status.payload.total,
            downloaded_portion: givenStatus.status.payload.downloaded_portion,
            eta_secs: givenStatus.status.payload.eta_secs
        }
    }
    return {
        filename,
        status,
        downloadSpeed: null,
        downloaded: null,
        total: null,
        downloaded_portion: null,
        eta_secs: null
    }
}
export interface DownloadTableProps {
    toRender: List<StatusFullResultValue>
}
export function DownloadTable({ toRender }: DownloadTableProps) {
    const requiredArr = useMemo(() => toRender.map((toMap) => convertStatusToView(toMap)).toArray(), [toRender])
    const columnHelper = createColumnHelper<StatusView>()
    const columns = [
        columnHelper.accessor("filename", {
            header: "Filename",
            cell: (info) => info.getValue(),
        }),
        columnHelper.accessor("status", {
            header: "Status",
            cell: (info) => info.getValue(),
        }),
        columnHelper.accessor("downloaded", {
            header: "Downloaded",
            cell: (info) => <Text>{info.getValue() != null ? `${bytesToMIB(info.getValue()!)} MIB` : "-"}</Text>,
        }),
        columnHelper.accessor("total", {
            header: "Total",
            cell: (info) => <Text>{info.getValue() != null ? `${bytesToMIB(info.getValue()!)} MIB` : "-"}</Text>,
        }),
        columnHelper.accessor("downloadSpeed", {
            header: "Download",
            cell: (info) => <Text>{info.getValue() != null ? `${info.getValue()?.toFixed(2)} MBPS` : "-"}</Text>,
        }),
        columnHelper.accessor("downloaded_portion", {
            header: "Downloaded Portion",
            cell: (info) => <Text>{info.getValue() != null ? `${(info.getValue()! * 100).toFixed(2)}%` : "-"}</Text>,
        }),
        columnHelper.accessor("eta_secs", {
            header: "ETA",
            cell: (info) => <Text>{info.getValue() != null ? `${formatTime(info.getValue()!)}` : "-"}</Text>,
        })
    ]
    const tableModel = useReactTable({
        data: requiredArr,
        columns: columns,
        getCoreRowModel: getCoreRowModel()
    })
    return (
        <Table>
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
                    <Table.Tr key={row.id}>
                        {row.getVisibleCells().map((cell) => (
                            <Table.Td key={cell.id}>
                                {flexRender(cell.column.columnDef.cell, cell.getContext())}
                            </Table.Td>
                        ))}
                    </Table.Tr>
                ))}
            </Table.Tbody>
        </Table>
    )
}
