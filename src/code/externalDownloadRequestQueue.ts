import { useEffect, useState, useRef } from "react";
import { ExternalDownloadRequest, ExternalDownloadRequestSchema } from "./typesOfBackend";
import { listen } from "@tauri-apps/api/event";
import { notifications } from "@mantine/notifications";
export type ExternalRequestMap = Map<string, ExternalDownloadRequest>;
export function useExternalDownloadRequestMap() {
    const [queue, setQueue] = useState<ExternalRequestMap>(new Map());
    useEffect(() => {
        const unlistenFnPromise = listen("external-download-request", (event) => {
            const uuid = crypto.randomUUID();
            const eventPayload = ExternalDownloadRequestSchema.safeParse(event.payload);
            if (!eventPayload.success) {
                console.error("Invalid external download request received:", event.payload);
                return;
            }
            setQueue((prevQueue) => new Map(prevQueue).set(uuid, eventPayload.data));
        });
        return () => { unlistenFnPromise.then((unlistenFn) => unlistenFn()) };
    }, [])
    return queue;
}

export function useNotifyDownload(requestMap: ExternalRequestMap) {
    const lastChecked = useRef(0);
    const queue = Array.from(requestMap.values());
    useEffect(() => {
        for (let i = lastChecked.current; i < queue.length; i++) {
            notifications.show({
                title: "New Download Request",
                message: `URL: ${queue[i].url}`,
                color: "blue",
            });
        }

        lastChecked.current = queue.length;
    }, [queue]);
}
