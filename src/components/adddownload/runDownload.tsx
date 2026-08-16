import { Box, TextInput, Flex, ActionIcon, Button, LoadingOverlay, NumberInput } from "@mantine/core";
import { useState } from "react";
import { Folder } from "lucide-react";
import { MoreInfo } from "./moreInfo.tsx";
import { useAddDownload, useOpenFile } from "../../code/useDownloads.ts";
import { DownloadInfo } from "../../code/typesOfBackend.ts";
import { joinPaths } from "../../code/utils.ts";
import { notifications } from "@mantine/notifications";

export interface RunDownloadDialogProps {
    metadata: DownloadInfo;
    onSuccess?: () => void;
}

export function RunDownloadDialog({ metadata, onSuccess }: RunDownloadDialogProps) {
    const [filepathToDownloadTo, setFilepathToDownloadTo] = useState("");
    const [filename, setFilename] = useState(metadata.filename ?? "");
    const [numberOfSegments, setNumberOfSegments] = useState(metadata.segmented_possible ? 4 : 1);

    const openFileMutation = useOpenFile();
    const addDownload = useAddDownload();

    const chooseFolderByPrompt = async () => {
        try {
            const folderPath = await openFileMutation.mutateAsync();
            if (folderPath) {
                setFilepathToDownloadTo(folderPath);
            }
        } catch (err) {
            notifications.show({
                title: "Folder Selection",
                message: "No folder was selected",
                color: "yellow",
            });
        }
    };

    const startDownloadNow = () => {
        const combinedFilepath = joinPaths(filepathToDownloadTo, filename);
        const effectiveSegments = metadata.segmented_possible ? numberOfSegments : 1;

        addDownload.mutate({
            url: metadata.url,
            numberOfSegments: effectiveSegments,
            filepath: combinedFilepath,
        },
            {
                onSuccess: onSuccess,
                onError: (err) => {
                    console.error(err)
                    notifications.show({
                        title: "Failed to add download",
                        message: err.message
                    })
                }
            }
        );

    };

    return (
        <Box pos="relative">
            <LoadingOverlay
                visible={openFileMutation.isPending || addDownload.isPending}
                zIndex={1000}
                overlayProps={{ radius: "sm", blur: 2 }}
            />

            <form onSubmit={(e) => {
                e.preventDefault();
                startDownloadNow();
            }}>
                <Flex direction="column" gap="md">
                    {/* 1. Filename Input with Label */}
                    <TextInput
                        label="File Name"
                        value={filename}
                        placeholder="e.g. linux-mint.iso"
                        onChange={(event) => setFilename(event.target.value)}
                        required
                    />

                    {/* 2. Filepath Input with EMBEDDED Folder Action Icon */}
                    <TextInput
                        label="Save Directory"
                        value={filepathToDownloadTo}
                        onChange={(event) => setFilepathToDownloadTo(event.target.value)}
                        placeholder="Select destination folder..."
                        required
                        rightSection={
                            <ActionIcon
                                variant="subtle"
                                color="gray"
                                onClick={chooseFolderByPrompt}
                                title="Browse folder"
                            >
                                <Folder size={18} />
                            </ActionIcon>
                        }
                    />

                    {/* 3. Segmented Threads Configuration */}
                    {metadata.segmented_possible && (
                        <NumberInput
                            label="Download Connections (Segments)"
                            description="Higher values can increase download speed on fast connections"
                            min={1}
                            max={32}
                            value={numberOfSegments}
                            onChange={(val) => setNumberOfSegments(typeof val === "number" ? val : 1)}
                        />
                    )}

                    {/* 4. Collapsible Info Card */}
                    <MoreInfo
                        url={metadata.url}
                        filename={metadata.filename}
                        filesize={metadata.filesize ? `${(metadata.filesize / 1024 / 1024).toFixed(1)} MiB` : "Unknown"}
                        segmentedPossible={metadata.segmented_possible}
                    />

                    {/* 5. Primary Action Button at the BOTTOM */}
                    <Button type="submit" fullWidth mt="xs">
                        Start Download
                    </Button>
                </Flex>
            </form>
        </Box>
    );
}
