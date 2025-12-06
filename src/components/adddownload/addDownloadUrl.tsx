import { Modal, Input, Text, Flex, LoadingOverlay, Button } from "@mantine/core";
import { DownloadInfoGetError } from "../../code/typesOfBackend";
import { useState } from "react";
export interface AddDownloadProps {
    open: boolean,
    onOpenStateChangeRequested: (open: boolean) => void
    loading: boolean,
    error?: DownloadInfoGetError,
    submitMetadata?: (url: string) => void;
}

export function AddDownloadUrl({ open, onOpenStateChangeRequested, submitMetadata, loading, error: err }: AddDownloadProps) {
    const [url, setUrl] = useState("");
    return (
        <Modal
            opened={open}
            onClose={() => {
                onOpenStateChangeRequested(false);
            }}
            title="Add Download"
            pos={"relative"}
        >
            <LoadingOverlay visible={loading} zIndex={1000} overlayProps={{ radius: "sm", blur: 2 }}>
            </LoadingOverlay>
            <form
                onSubmit={(e) => {
                    e.preventDefault();
                    submitMetadata?.(url);
                }}
            >
                <Flex direction="column" gap="xs">
                    <Input type="url" placeholder="Url" value={url} onChange={(e) => setUrl(e.target.value)} />
                    {err?.payload && <Text c={"red"}>{err.payload}</Text>}
                    <Button type="submit">Download</Button>
                </Flex>
            </form>
        </Modal>
    )
}
