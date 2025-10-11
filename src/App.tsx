import { useState } from "react";
import "./App.css";
import "@mantine/core/styles.css"
import { MantineProvider,createTheme,ColorSchemeScript } from "@mantine/core"
import  { DownloadHeader } from "./components/header/downloaditemheader"
import { AddDownloadDialog } from "./components/adddownload/adddownload"
const theme = createTheme({
})


function App() {
    const [downloadDialogOpen, changeOpenState] = useState(false)
    return (
        <>
            <ColorSchemeScript forceColorScheme="dark" />
            <MantineProvider theme={theme} forceColorScheme="dark">
                <DownloadHeader onClickAddDownload={()=>changeOpenState(true)} />
                <AddDownloadDialog open={downloadDialogOpen} onOpenStateChangeRequested={changeOpenState} />
            </MantineProvider>
        </>
    )
}

export default App;
