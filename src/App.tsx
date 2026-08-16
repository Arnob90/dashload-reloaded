import "./App.css";
import "@mantine/core/styles.css"
import { MantineProvider, Flex, createTheme } from "@mantine/core"
import { DownloadHeader } from "./components/header/downloaditemheader"
import "@mantine/notifications/styles.css"
import { Notifications } from "@mantine/notifications"
import { AddDownload } from "./components/adddownload/addDownloadMain";
import { useDisclosure } from "@mantine/hooks";
import { DownloadTable } from "./components/downloadtable/downloadtable";
import { ErrorBoundary, getErrorMessage } from "react-error-boundary"
const theme = createTheme({
})

function App() {
    const [addDownloadDialogOpened, { open: openAddDownloadDialog, close: closeAddDownloadDiagog }] = useDisclosure(false)
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
                    <AddDownload opened={addDownloadDialogOpened} onClose={closeAddDownloadDiagog} />
                    <DownloadTable />
                </Flex>
            </MantineProvider>
        </ErrorBoundary>
    )
}

export default App;
