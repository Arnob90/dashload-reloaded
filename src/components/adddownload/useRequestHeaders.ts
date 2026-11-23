import { useAppConfig } from "../../code/useAppConfig";
import { RequestHeaders } from "../../code/typesOfBackend";
import { useEffect, useState } from "react";
export function useRequestHeaders(defaultHeaders?: RequestHeaders) {
    const config = useAppConfig();
    const headers = defaultHeaders ?? config.request_headers;
    const [requestHeaders, setRequestHeaders] = useState<RequestHeaders>(headers);
    useEffect(() => {
        for (const key in requestHeaders) {
            if (requestHeaders[key as keyof RequestHeaders] === "") {
                setRequestHeaders(prevHeaders => {
                    const newHeaders = { ...prevHeaders };
                    newHeaders[key as keyof RequestHeaders] = null;
                    return newHeaders;
                })
            }
        }
    }, [requestHeaders])
    return [
        requestHeaders,
        setRequestHeaders,
    ] as const;
}