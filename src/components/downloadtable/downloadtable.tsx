import { useMemo } from "react"
import { GetStatusFullResult } from "../../code/typesofbackend"
import { List } from "immutable"
import { ColumnDef, createColumnHelper } from "@tanstack/react-table"
export type StatusFullResultValue = GetStatusFullResult[keyof GetStatusFullResult]
export function DownloadTable(toRender: List<StatusFullResultValue>) {
    const toRenderArr = useMemo(() => toRender.toJS(), [toRender])
    const columnHelper = createColumnHelper<StatusFullResultValue>()
    const columns: ColumnDef<StatusFullResultValue>[] = [
    ]
}
