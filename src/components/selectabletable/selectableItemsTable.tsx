import styles from "./selectableitemstable.module.css"
import { Table, Flex } from "@mantine/core"
import { useEffect } from "react";

type SelectableItemsTableProps = React.ComponentProps<typeof Table> & {
    setSelectedId: (toSet: string | null) => void;
    children: React.ReactNode
}

export function SelectableItemsTable({ children, setSelectedId, ...rest }: SelectableItemsTableProps) {
    useEffect(() => {
        const handleEscapeKeyPressed = (event: KeyboardEvent) => {
            if (event.key === "Escape") {
                setSelectedId(null)
            }
        }
        window.addEventListener("keydown", handleEscapeKeyPressed);
        return () => {
            window.removeEventListener("keydown", handleEscapeKeyPressed);
        }
    }, [])
    return (
        <Flex onClick={(e) => {
            if (e.target === e.currentTarget) {
                setSelectedId(null)
            }
        }} w={"100%"} h={"100%"} direction={"column"}>
            <Table {...rest} className={styles.item}>
                {children}
            </Table>
        </Flex>
    )
}
