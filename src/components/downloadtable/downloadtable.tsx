import { useMemo, useState } from "react"
import { GetStatusFullResult } from "../../code/typesOfBackend.ts"
import { List } from "immutable"
import { Table, Text } from "@mantine/core"
import { bytesToMIB } from "../../code/utils.ts"
import { createColumnHelper, getCoreRowModel, useReactTable, flexRender } from "@tanstack/react-table"
import style from "./downloadtable.module.css"
import { SelectableItemsTable } from "../selectabletable/selectableItemsTable.tsx"
export type StatusFullResultValue = GetStatusFullResult[keyof GetStatusFullResult]
export type StatusViewWithoutId = {
    filename: string,
    status: string,
    downloaded: number | null,
    total: number | null,
    downloadSpeed: number | null,
    downloaded_portion: number | null,
    eta_secs: number | null
}
export type StatusView = StatusViewWithoutId & {
    id: string
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
function convertStatusToView(givenStatus: StatusFullResultValue): StatusViewWithoutId {
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
function convertStatusDictToView(givenStatus: GetStatusFullResult): List<StatusView> {
    let requiredArr = List<StatusView>([])
    for (const id in givenStatus) {
        const result = givenStatus[id]
        const view = convertStatusToView(result)
        requiredArr = requiredArr.push({ ...view, id: id })
    }
    return requiredArr
}
export interface DownloadTableProps {
    toRender: GetStatusFullResult
}
export function DownloadTable({ toRender }: DownloadTableProps) {
    const requiredArr = useMemo(() => convertStatusDictToView(toRender).toArray(), [toRender])
    const [selectedId, setSelectedId] = useState<string | null>("")
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
                    <Table.Tr key={row.id} tabIndex={0} onClick={() => {
                        if (selectedId === row.original.id) {
                            setSelectedId(null)
                            return
                        }
                        setSelectedId(row.original.id)
                    }} className={row.original.id === selectedId ? `${style.selected_row}` : `${style.table_row}`}>
                        {row.getVisibleCells().map((cell) => (
                            <Table.Td key={cell.id}>
                                {flexRender(cell.column.columnDef.cell, cell.getContext())}
                            </Table.Td>
                        ))}
                    </Table.Tr>
                ))}
            </Table.Tbody>
        </SelectableItemsTable>
    )
}
