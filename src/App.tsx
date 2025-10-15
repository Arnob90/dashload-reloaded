import { useEffect, useMemo, useState } from "react";
import "./App.css";
import "@mantine/core/styles.css"
import { MantineProvider, createTheme, ColorSchemeScript, Flex } from "@mantine/core"
import { DownloadHeader } from "./components/header/downloaditemheader"
import { AddDownloadDialog } from "./components/adddownload/adddownload"
import { useDownloadStatuses } from "./code/getStatus";
import { DownloadTable } from "./components/downloadtable/downloadtable";
import { List } from "immutable"
import { StatusFullResultValue } from "./components/downloadtable/downloadtable";
const theme = createTheme({
})

function App() {
    const [downloadDialogOpen, changeOpenState] = useState(false)
    const { data, isLoading, error } = useDownloadStatuses()
    const requiredDataList: List<StatusFullResultValue> = useMemo(() => {
        if (data) {
            return List(Object.values(data))
        }
        return List<StatusFullResultValue>([])
    }, [data])
    return (
        <>
            <ColorSchemeScript forceColorScheme="dark" />
            <MantineProvider theme={theme} forceColorScheme="dark">
                <DownloadHeader onClickAddDownload={() => changeOpenState(true)} />
                <AddDownloadDialog open={downloadDialogOpen} onOpenStateChangeRequested={changeOpenState} />
                <Flex>
                    <DownloadTable toRender={requiredDataList}></DownloadTable>
                </Flex>
            </MantineProvider>
        </>
    )
}

export default App;
