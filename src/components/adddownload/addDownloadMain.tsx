import { useState } from "react";
import { Modal } from "@mantine/core";
import { DownloadInfo } from "../../code/typesOfBackend";
import { AddDownloadUrl } from "./addDownloadUrl";
import { RunDownloadDialog } from "./runDownload";

interface AddDownloadProps {
    opened: boolean,
    onClose: () => void
}

export function AddDownload({ opened, onClose }: AddDownloadProps) {
    const [currentMetadata, setMetadata] = useState<DownloadInfo | undefined>()
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
                }} />
            )}
        </Modal>
    )
}
