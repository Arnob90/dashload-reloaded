import { ActionIcon, Flex, TextInput } from "@mantine/core"
import styles from "./header.module.css"
import { Plus, Search, Trash } from "lucide-react"
import { useMantineTheme } from "@mantine/core"
import { useState } from "react";

export interface DownloadHeaderProps {
    onClickAddDownload?: ()=>void;
    onClickRemoveDownload?: ()=>void;
    onClickSearch?: (query:String)=>void;
}

export function DownloadHeader({onClickAddDownload,onClickRemoveDownload,onClickSearch}:DownloadHeaderProps) {
    const [textInputContents,setTextInputContents] = useState("")
    return (
        <Flex direction="column">
            <Flex w="100%" className={styles.container} align="center" m="2vh" gap="md">
                <ActionIcon size="xl" onClick={onClickAddDownload}>
                    <Plus size="2vh"></Plus>
                </ActionIcon>
                <ActionIcon size="xl" color="#e00004" onClick={onClickRemoveDownload}>
                    <Trash size="2vh"></Trash>
                </ActionIcon>
                <Flex ml="auto" mr="xl" gap="md">
                    <TextInput value={textInputContents} onChange={(event)=>{
                        setTextInputContents(event.target.value)
                    }} />
                    <ActionIcon color="gray" size="lg" onClick={()=>onClickSearch?.(textInputContents)}>
                        <Search></Search>
                    </ActionIcon>
                </Flex>
            </Flex>
            <div className={styles.separator}></div>
        </Flex>
    )
}
