import { GetStatusFullResult } from "../../code/typesOfBackend"
export type StatusFullResultValue = GetStatusFullResult[keyof GetStatusFullResult]
import { List } from "immutable"
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
export function convertStatusDictToView(givenStatus: GetStatusFullResult): List<StatusView> {
    let requiredArr = List<StatusView>([])
    for (const id in givenStatus) {
        const result = givenStatus[id]
        const view = convertStatusToView(result)
        requiredArr = requiredArr.push({ ...view, id: id })
    }
    return requiredArr
}
