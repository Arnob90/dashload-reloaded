import { AddDownloadUrl } from "./addDownloadUrl";
import { useState } from "react";
import { DownloadInfo, getErrorMessageFromError } from "../../code/typesOfBackend";
import { RunDownloadDialog } from "./runDownload";
import { getDownloadInfo } from "../../code/getDownloadInfo";
import { notifications } from "@mantine/notifications";

interface AddDownloadProps {
    open: boolean
    onOpenStateChangeRequested: (open: boolean) => void
}

export function AddDownload({ open, onOpenStateChangeRequested }: AddDownloadProps) {
    const [currentMetadata, setMetadata] = useState<DownloadInfo | undefined>()
    const [loading, setLoading] = useState(false)
    const submit = async (url: string) => {
        setLoading(true)
        const info = await getDownloadInfo(url)
        if ("kind" in info) {
            console.log(info.kind)
            notifications.show({
                message: getErrorMessageFromError(info),
                title: "Error",
            })
            setLoading(false)
        }
        else {
            setMetadata(info)
        }
    }
    const setOpenIntermediate = (toSetTo: boolean) => {
        if (!toSetTo) {
            setMetadata(undefined)
            setLoading(false)
        }
        onOpenStateChangeRequested(toSetTo)
    }
    return (
        currentMetadata === undefined ? (
            <AddDownloadUrl onOpenStateChangeRequested={setOpenIntermediate} open={open} submitMetadata={submit} loading={loading} />
        ) :
            (
                <RunDownloadDialog metadata={currentMetadata} open={open} onOpenStateChangeRequested={setOpenIntermediate} />
            )

    )
}
