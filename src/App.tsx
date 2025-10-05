import { useState } from "react";
import reactLogo from "./assets/react.svg";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";
import "@mantine/core/styles.css"
import styles from "./App.module.css"
import { Button, ColorSchemeScript, createTheme, MantineProvider } from "@mantine/core";
const theme = createTheme({
})


function App() {
    return (
        <>
            <ColorSchemeScript forceColorScheme="dark" />
            <MantineProvider theme={theme} forceColorScheme="dark">
                <div className={styles.flex}>
                </div>
            </MantineProvider>
        </>
    )
}

export default App;
