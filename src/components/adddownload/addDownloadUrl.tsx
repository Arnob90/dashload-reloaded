import { Input, Text, Flex, LoadingOverlay, Button } from "@mantine/core";
import { DownloadInfo, DownloadInfoGetApiError, DownloadInfoGetApiErrorSchema } from "../../code/typesOfBackend";
import { useState } from "react";
import { useFetchMetadata } from "../../code/useDownloads";
import { notifications } from "@mantine/notifications";
import { CollapseSection } from "./collapseSection";
import { useDisclosure } from "@mantine/hooks";
import { RequestHeadersSection } from "./requestHeadersSection";
import { useRequestHeaders } from "./useRequestHeaders";
export interface AddDownloadProps {
    error?: DownloadInfoGetApiError,
    submitMetadata?: (info: DownloadInfo) => void;
}


export function AddDownloadUrl({ submitMetadata, error: err }: AddDownloadProps) {
    const [url, setUrl] = useState("");
    const getDownloadMetadataMutation = useFetchMetadata()
    const [expanded, { toggle }] = useDisclosure(false);
    const [requestHeaders, setRequestHeaders] = useRequestHeaders();
    const submit = async () => {
        try {
            const metadata = await getDownloadMetadataMutation.mutateAsync({ url, requestHeaders })
            submitMetadata?.(metadata)
        }
        catch (err) {
            const errParsed = DownloadInfoGetApiErrorSchema.parse(err)
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
                    <Input
            type="url"
            placeholder="Url"
            value={url}
            onChange={(e) => setUrl(e.target.value)}
            variant="filled"
            radius="md"
            styles={{
                input: {
                    backgroundColor: "rgba(255, 255, 255, 0.05)",
                    border: "1px solid rgba(255, 255, 255, 0.08)",
                    transition: "border-color 150ms ease, background-color 150ms ease",
                },
            }}
        />
                    {err?.payload && <Text c={"red"}>{err.payload}</Text>}
                    <Button type="submit" variant="filled" color="indigo" radius="md" fullWidth>
            Download
        </Button>
                </Flex>
                <CollapseSection label="Advanced Options" toggleFunction={toggle} opened={expanded}>
                    <RequestHeadersSection requestHeaders={requestHeaders} setRequestHeaders={setRequestHeaders} />
                </CollapseSection>
            </form>
        </>
    )
}
