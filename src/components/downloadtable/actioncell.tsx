import { usePause, useResume } from "../../code/download.ts"
import { notifications } from "@mantine/notifications"
import { ChevronRight, Pause } from "lucide-react"
import { StatusView } from "./statusView.tsx"
import { ActionIcon } from "@mantine/core"
export interface ActionCellProps {
    givenView: StatusView
}
export function ActionCell({ givenView }: ActionCellProps) {
    const pause = usePause()
    const resume = useResume()
    const handlePause = async (id: string) => {
        const err_maybe = await pause.mutateAsync({ id: id })
        if (err_maybe === undefined) {
            return
        }
        console.log(err_maybe)
        notifications.show({
            title: "Failed to Pause",
            message: err_maybe.payload
        })
    }
    const handleResume = async (id: string) => {
        const err_maybe = await resume.mutateAsync({ id: id })
        if (err_maybe === undefined) {
            return
        }
        console.log(err_maybe)
        const err_thing = err_maybe.payload;
        if (typeof err_thing === "string") {
            notifications.show({
                title: "Failed to Pause",
                message: err_thing
            })
        }
        else {
            const message = err_thing.payload;
            notifications.show({
                title: "Failed to Resume",
                message
            })
        }
    }
    const status = givenView.status;
    if (status === "Paused") {
        return <ActionIcon variant="subtle" onClick={() => handleResume(givenView.id)}>
            <ChevronRight></ChevronRight>
        </ActionIcon>
    }
    return (
        <ActionIcon variant="subtle" onClick={() => handlePause(givenView.id)}>
            <Pause></Pause>
        </ActionIcon>
    )
}
