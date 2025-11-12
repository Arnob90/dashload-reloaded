import { useState } from "react";
import "./App.css";
import "@mantine/core/styles.css"
import { MantineProvider, createTheme, ColorSchemeScript, Flex } from "@mantine/core"
import { DownloadHeader } from "./components/header/downloaditemheader"
import "@mantine/notifications/styles.css"
import { useDownloadStatuses } from "./code/getStatus";
import { DownloadTable } from "./components/downloadtable/downloadtable";
import { Notifications } from "@mantine/notifications"
import { AddDownload } from "./components/adddownload/addDownloadMain";
const theme = createTheme({
})

function App() {
    const [downloadDialogOpen, setOpen] = useState(false)
    const { data } = useDownloadStatuses()
    return (
        <>
            <ColorSchemeScript forceColorScheme="dark" />
            <MantineProvider theme={theme} forceColorScheme="dark">
                <Notifications />
                <DownloadHeader onClickAddDownload={() => { setOpen(true); console.log(downloadDialogOpen) }} />
                <AddDownload open={downloadDialogOpen} onOpenStateChangeRequested={(toSet) => {
                    setOpen(toSet)
                }} />
                <Flex h="100vh">
                    {data === undefined ? undefined : <DownloadTable toRender={data}></DownloadTable>}
                </Flex>
            </MantineProvider>
        </>
    )
}

export default App;
