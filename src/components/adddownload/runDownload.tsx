import { Modal, Box, TextInput, Flex, ActionIcon, Button, Text, LoadingOverlay, NumberInput } from "@mantine/core";
import { DownloadInfo, getErrorMessageFromError } from "../../code/typesOfBackend";
import { useState } from "react";
import { Folder } from "lucide-react";
import { CollapsibleSection } from "./extraOptions";
import { chooseFolder } from "../../code/chooseFolder";
import { joinPaths } from "../../code/joinPath.ts";
import { useStartDownloadSegmented } from "../../code/download.ts"
import { notifications } from "@mantine/notifications";

export interface RunDownloadDialogProps {
    metadata: DownloadInfo,
    open: boolean,
    onOpenStateChangeRequested: (open: boolean) => void
}

export function RunDownloadDialog({ metadata, open, onOpenStateChangeRequested }: RunDownloadDialogProps) {
    const [filepathToDownloadTo, setFilepathToDownloadTo] = useState("")
    const [filename, setFilename] = useState(metadata.filename ?? "")
    const [loading, setLoading] = useState(false)
    const [numberOfSegments, setNumberOfSegments] = useState(metadata.segmented_possible ? 3 : 1)
    const startDownloadSegmented = useStartDownloadSegmented()
    const chooseFolderByPrompt = async () => {
        const filepathChoosen = await chooseFolder()
        if (filepathChoosen === null) {
            return
        }
        setFilepathToDownloadTo(filepathChoosen)
    }
    const startDownloadNow = async () => {
        const combinedFilepath = await joinPaths(filepathToDownloadTo, filename)
        console.log(combinedFilepath)
        setLoading(true)
        const res = await startDownloadSegmented.mutateAsync({ url: metadata.url, filepath: combinedFilepath, numberOfSegments: numberOfSegments })
        if (res !== undefined) {
            notifications.show({
                message: getErrorMessageFromError(res),
                title: "Error",
            })
        }
        setLoading(false)
        onOpenStateChangeRequested(false)
    }
    return (
        <Modal opened={open} onClose={() => { onOpenStateChangeRequested(false) }} title="Download Info" pos={"relative"}>
            <LoadingOverlay visible={loading} zIndex={1000} overlayProps={{ radius: "sm", blur: 2 }} />
            <form onSubmit={(e) => {
                e.preventDefault()
                startDownloadNow()
            }}>
                <Flex direction={"column"} w={"100%"} gap={0.5}>
                    <TextInput value={filename} placeholder="Filename" onChange={(event) => setFilename(event.target.value)} />
                    <Flex my={"sm"} w={"100%"} gap={"sm"}>
                        <TextInput value={filepathToDownloadTo} onChange={(event) => {
                            setFilepathToDownloadTo(event.target.value)
                        }} placeholder="Filepath" w={"100%"} />
                        <ActionIcon size={"5vh"} color="gray" onClick={chooseFolderByPrompt}>
                            <Folder color="#9c9c9c" />
                        </ActionIcon>
                    </Flex>
                    <Button type="submit" disabled={filepathToDownloadTo === "" || filename === ""}>Start Download</Button>
                    <CollapsibleSection optionText="Info">
                        <Flex bg={"dark"} direction={"column"}>
                            <Box m={"sm"}>
                                <Text>Filename: {metadata.filename}</Text>
                                <Text>Filesize: {metadata.filesize}</Text>
                                <Text>Segmented: {metadata.segmented_possible ? "Yes" : "No"}</Text>
                            </Box>
                        </Flex>
                    </CollapsibleSection>
                    <CollapsibleSection optionText="Advanced Options">
                        <Box m={"sm"} bg={"dark"}>
                            <Text>Number of Segments</Text>
                            <NumberInput min={1} max={100} disabled={!metadata.segmented_possible} value={numberOfSegments} onChange={(num) => {
                                if (typeof num === "string") {
                                    const parsed = Number(num)
                                    setNumberOfSegments(parsed)
                                    return
                                }
                                setNumberOfSegments(num)

                            }}></NumberInput>
                        </Box>
                    </CollapsibleSection>
                </Flex>
            </form>
        </Modal >
    );
}
