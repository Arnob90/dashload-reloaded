import { getCurrentWindow } from "@tauri-apps/api/window"
import { useEffect } from "react"
export function useCloseToTray() {
    useEffect(() => {
        const appWindow = getCurrentWindow()
        const unlistenPromise = appWindow.onCloseRequested(async (event) => {
            event.preventDefault()
            await appWindow.hide()
        })
        return () => {
            unlistenPromise.then((unlisten) => unlisten())
        }
    })
}
