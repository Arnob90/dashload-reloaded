import { ActionIcon, Flex, TextInput } from "@mantine/core"
import styles from "./header.module.css"
import { Plus, Search } from "lucide-react"
import { useState } from "react";

export interface DownloadHeaderProps {
    onClickAddDownload?: () => void;
    onClickSearch?: (query: String) => void;
}

export function DownloadHeader({ onClickAddDownload, onClickSearch }: DownloadHeaderProps) {
    const [textInputContents, setTextInputContents] = useState("")
    return (
        <Flex direction="column">
            <Flex w="100%" className={styles.container} align="center" m="2vh" gap="md">
                <ActionIcon size="xl" onClick={onClickAddDownload}>
                    <Plus size="2vh"></Plus>
                </ActionIcon>
                <Flex ml="auto" mr="xl" gap="md">
                    <TextInput value={textInputContents} onChange={(event) => {
                        setTextInputContents(event.target.value)
                    }} />
                    <ActionIcon color="gray" size="lg" onClick={() => onClickSearch?.(textInputContents)}>
                        <Search></Search>
                    </ActionIcon>
                </Flex>
            </Flex>
            <div className={styles.separator}></div>
        </Flex>
    )
}
