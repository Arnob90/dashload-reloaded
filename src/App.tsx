import "./App.css";
import "@mantine/core/styles.css"
import { MantineProvider, Flex, createTheme } from "@mantine/core"
import { DownloadHeader } from "./components/header/downloaditemheader"
import { useExternalDownloadRequestMap, useNotifyDownload } from "./code/externalDownloadRequestQueue"
import "@mantine/notifications/styles.css"
import { Notifications } from "@mantine/notifications"
import { AddDownload } from "./components/adddownload/addDownloadMain";
import { useDisclosure } from "@mantine/hooks";
import { DownloadTable } from "./components/downloadtable/downloadtable";
import { ErrorBoundary, getErrorMessage } from "react-error-boundary"
import { ExtensionRequestDownload } from "./components/adddownload/addDownloadOnExtensionRequest";
const theme = createTheme({
    fontFamily: 'Inter, ui-sans-serif, system-ui, -apple-system, "Segoe UI", Roboto, sans-serif',
    primaryColor: 'indigo',
    defaultRadius: 'md',
    colors: {
        indigo: [
            '#eef2ff', '#e0e7ff', '#c7d2fe', '#a5b4fc',
            '#818cf8', '#6366f1', '#4f46e5', '#4338ca',
            '#3730a3', '#312e81',
        ],
    },
    shadows: {
        sm: '0 1px 2px rgba(0,0,0,0.3)',
        md: '0 4px 12px rgba(0,0,0,0.4)',
        lg: '0 12px 40px rgba(0,0,0,0.5)',
    },
    other: {
        glassBg: 'rgba(24, 26, 32, 0.7)',
        glassBorder: 'rgba(255,255,255,0.08)',
    },
});
function App() {
    const [addDownloadDialogOpened, { open: openAddDownloadDialog, close: closeAddDownloadDialog }] = useDisclosure(false)
    const requestMap = useExternalDownloadRequestMap();
    useNotifyDownload(requestMap);
    return (
        <ErrorBoundary fallbackRender={({ error, resetErrorBoundary }) => (
            <div role="alert">
                <p>Something went wrong:</p>
                <pre>{getErrorMessage(error)}</pre>
                <button onClick={resetErrorBoundary}>Try again</button>
            </div>
        )}>
            <MantineProvider theme={theme} forceColorScheme="dark">
                <Flex direction="column" h="100vh" w="100vw">
                    <Notifications />
                    <DownloadHeader onClickAddDownload={openAddDownloadDialog} />
                    <AddDownload opened={addDownloadDialogOpened} onClose={closeAddDownloadDialog} />
                    <ExtensionRequestDownload requestMap={requestMap} />
                    <DownloadTable />
                </Flex>
            </MantineProvider>
        </ErrorBoundary>
    )
}

export default App;
