import { useMemo, useState } from "react"
import { GetStatusFullResult } from "../../code/typesOfBackend.ts"
import { bytesToMIB } from "../../code/utils.ts"
import { createColumnHelper, getCoreRowModel, useReactTable, flexRender } from "@tanstack/react-table"
import style from "./downloadtable.module.css"
import { SelectableItemsTable } from "../selectabletable/selectableItemsTable.tsx"
import { convertStatusDictToView, StatusView } from "./statusView.tsx"
import { formatTime } from "../../code/formatTime"
import { ActionCell } from "./actioncell.tsx"
import { Text, Table } from "@mantine/core"
export interface DownloadTableProps {
    toRender: GetStatusFullResult
}
export function DownloadTable({ toRender }: DownloadTableProps) {
    const requiredArr = useMemo(() => convertStatusDictToView(toRender).toArray(), [toRender])
    const [selectedId, setSelectedId] = useState<string | null>("")
    const columnHelper = createColumnHelper<StatusView>()
    const columns = useMemo(() => [
        columnHelper.display({
            id: "actions",
            header: "",
            cell: (info) => <ActionCell givenView={info.row.original}></ActionCell>
        }),
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
    ], [])
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
