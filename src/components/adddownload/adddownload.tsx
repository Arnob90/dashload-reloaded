import { Flex, Modal, TextInput, ActionIcon, Button } from "@mantine/core"
import { useState } from "react";
import { Folder } from "lucide-react";
import styles from "./adddownloaditem.module.css"
import { invoke } from "@tauri-apps/api/core";
import { startDownload } from "../../code/download";

export interface AddDownloadDialogProps {
    open: boolean,
    onOpenStateChangeRequested: (open: boolean) => void;
}


export function AddDownloadDialog({ open, onOpenStateChangeRequested }: AddDownloadDialogProps) {
    const [url, setUrl] = useState("")
    const [filepath, setFilepath] = useState("")
    return (
        <Modal opened={open} onClose={() => { onOpenStateChangeRequested(false) }} title="Add Download">
            <Flex direction="column" gap="sm">
                <TextInput placeholder="URL" type="url" onChange={(event) => {
                    setUrl(event.target.value)
                }} value={url} />
                <Flex gap="sm">
                    <TextInput placeholder="Filepath" value={filepath} onChange={(event) => {
                        setFilepath(event.target.value)
                    }} className={styles.text_input} />
                    <ActionIcon size="lg" color="#3d3d3d" onClick={async () => {
                        let filepath_choosen = await invoke<string | null>("open_file")
                        if (filepath_choosen != null) {
                            setFilepath(filepath_choosen)
                        }
                    }}>
                        <Folder color="gray" />
                    </ActionIcon>
                </Flex>
                <Button onClick={() => {
                    startDownload(url, filepath)
                }}>Download</Button>
            </Flex>
        </Modal>
    )
}
