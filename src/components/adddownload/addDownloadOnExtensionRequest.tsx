import { useEffect, useState } from "react";
import { ExternalRequestMap } from "../../code/externalDownloadRequestQueue";
import { RunDownloadDialog } from "./runDownload";
import { Box, LoadingOverlay, Modal } from "@mantine/core";
import { useDownloadMetadata } from "../../code/useDownloads";
import { DEFAULT_REQUEST_HEADERS } from "../../code/typesOfBackend";
import { notifications } from "@mantine/notifications";

interface ExtensionRequestDownloadProps {
    requestMap: ExternalRequestMap
}

export function ExtensionRequestDownload({ requestMap }: ExtensionRequestDownloadProps) {
    const [modalClosed, setModalClosed] = useState(requestMap.size === 0);

    useEffect(() => {
        setModalClosed(false);
    }, [requestMap]);

    const open = !modalClosed && requestMap.size > 0;
    const reqArray = open ? Array.from(requestMap.values()) : undefined;
    const lastReq = reqArray ? reqArray[reqArray.length - 1] : undefined;
    
    const {
        data,
        isLoading,
        isError,
        error,
    } = useDownloadMetadata(lastReq?.url ?? "");

    const [headers, setHeaders] = useState(DEFAULT_REQUEST_HEADERS);
    
    useEffect(() => {
        setHeaders(lastReq?.headers ?? DEFAULT_REQUEST_HEADERS);
    }, [lastReq]);

    useEffect(() => {
        if (isError) {
            notifications.show({
                title: "Error handling download request",
                message: error?.message ?? "Unknown error",
                color: "red"
            });
            setModalClosed(true);
        }
    }, [isError, error]);

    const handleClose = () => {
        setModalClosed(true);
    };

    return (
        <Modal opened={open && !isError} onClose={handleClose} title="Add Download" size="lg" centered>
            <Box pos="relative" mih={100}>
                <LoadingOverlay visible={isLoading} zIndex={1000} />
                {data && !error && <RunDownloadDialog metadata={data} onSuccess={handleClose} headers={headers} setHeaders={setHeaders} />}
            </Box>
        </Modal>
    );
}
