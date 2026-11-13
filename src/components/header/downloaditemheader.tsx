import { Button, Flex, TextInput } from "@mantine/core"
import styles from "./header.module.css"
import { Plus, Search } from "lucide-react"
import { useState } from "react";

export interface DownloadHeaderProps {
    onClickAddDownload?: () => void;
    onClickSearch?: (query: String) => void;
}

export function DownloadHeader({ onClickAddDownload, onClickSearch }: DownloadHeaderProps) {
    const [textInputContents, setTextInputContents] = useState("");

    const handleKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
        if (e.key === "Enter") {
            onClickSearch?.(textInputContents);
        }
    };

    return (
        <Flex direction="column">
            <Flex w="100%" className={styles.container} align="center" justify="space-between">
                <Button
                    variant="filled"
                    color="indigo"
                    radius="xl"
                    leftSection={<Plus size={16} />}
                    onClick={onClickAddDownload}
                >
                    New Download
                </Button>

                <TextInput
                    placeholder="Search downloads..."
                    value={textInputContents}
                    onChange={(event) => setTextInputContents(event.target.value)}
                    onKeyDown={handleKeyDown}
                    leftSection={<Search size={16} />}
                    variant="filled"
                    radius="xl"
                    w={320}
                    styles={{
                        input: {
                            backgroundColor: "rgba(255, 255, 255, 0.05)",
                            border: "1px solid rgba(255, 255, 255, 0.08)",
                            transition: "border-color 150ms ease, background-color 150ms ease",
                        },
                    }}
                />
            </Flex>
            <div className={styles.separator}></div>
        </Flex>
    )
}