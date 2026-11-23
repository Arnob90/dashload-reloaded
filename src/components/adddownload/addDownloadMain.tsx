import { useState } from "react";
import { Modal } from "@mantine/core";
import { DownloadInfo, RequestHeaders } from "../../code/typesOfBackend";
import { useAppConfig } from "../../code/useAppConfig";
import { AddDownloadUrl } from "./addDownloadUrl";
import { RunDownloadDialog } from "./runDownload";

interface AddDownloadProps {
    opened: boolean,
    onClose: () => void
}

export function AddDownload({ opened, onClose }: AddDownloadProps) {
    const config = useAppConfig();
    const [currentMetadata, setMetadata] = useState<DownloadInfo | undefined>()
    const [headers, setHeaders] = useState<RequestHeaders>(config.request_headers)
    return (
        <Modal
            opened={opened}
            onClose={onClose}
            title="Add Download"
            pos={"relative"}
        >
            {currentMetadata === undefined ? (
                <AddDownloadUrl submitMetadata={setMetadata} />
            ) : (
                <RunDownloadDialog metadata={currentMetadata} onSuccess={() => {
                    setMetadata(undefined)
                    onClose()
                }} headers={headers} setHeaders={setHeaders} />
            )}
        </Modal>
    )
}