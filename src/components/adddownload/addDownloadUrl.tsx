import { Input, Text, Flex, LoadingOverlay, Button } from "@mantine/core";
import { DownloadInfo, DownloadInfoGetError, DownloadInfoGetErrorSchema } from "../../code/typesOfBackend";
import { useState } from "react";
import { useFetchMetadata } from "../../code/useDownloads";
import { notifications } from "@mantine/notifications";
export interface AddDownloadProps {
    error?: DownloadInfoGetError,
    submitMetadata?: (info: DownloadInfo) => void;
}

export function AddDownloadUrl({ submitMetadata, error: err }: AddDownloadProps) {
    const [url, setUrl] = useState("");
    const getDownloadMetadataMutation = useFetchMetadata()
    const submit = async () => {
        try {
            const metadata = await getDownloadMetadataMutation.mutateAsync(url)
            submitMetadata?.(metadata)
        }
        catch (err) {
            const errParsed = DownloadInfoGetErrorSchema.parse(err)
            notifications.show({
                title: "Download Failed",
                message: `Failed to get information. Kind:${errParsed.kind} ,Reason: ${errParsed.message}`
            })
        }
    }
    return (
        <>
            <LoadingOverlay visible={getDownloadMetadataMutation.isPending} zIndex={1000} overlayProps={{ radius: "sm", blur: 2 }}>
            </LoadingOverlay>
            <form
                onSubmit={(e) => {
                    e.preventDefault();
                    submit()
                }}
            >
                <Flex direction="column" gap="xs">
                    <Input type="url" placeholder="Url" value={url} onChange={(e) => setUrl(e.target.value)} />
                    {err?.payload && <Text c={"red"}>{err.payload}</Text>}
                    <Button type="submit">Download</Button>
                </Flex>
            </form>
        </>
    )
}
