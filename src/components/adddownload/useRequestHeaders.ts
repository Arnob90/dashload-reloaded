import { DEFAULT_REQUEST_HEADERS } from "../../code/defaults";
import { RequestHeaders } from "../../code/typesOfBackend";
import { useEffect, useState } from "react";
export function useRequestHeaders(defaultHeaders: RequestHeaders = DEFAULT_REQUEST_HEADERS) {
    const [requestHeaders, setRequestHeaders] = useState<RequestHeaders>(defaultHeaders);
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
