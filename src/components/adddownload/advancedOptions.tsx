import { Collapse, Text, ActionIcon, NumberInput, Box, Flex } from "@mantine/core"
import { ChevronDown, ChevronRight } from "lucide-react"
import { useDisclosure } from "@mantine/hooks"

export interface AdvancedOptionsProps {
    segmentedPossible: boolean,
    numberOfSegments: number,
    setNumberOfSegments: (num: number) => void
}

export function AdvancedOptions({ segmentedPossible, numberOfSegments, setNumberOfSegments }: AdvancedOptionsProps) {
    const [expanded, { toggle }] = useDisclosure(false)
    return (
        <Flex direction="column">

            <ActionIcon onClick={toggle}>
                {expanded ? <ChevronDown /> : <ChevronRight />}
            </ActionIcon>
            <Collapse expanded={expanded}>
                <Box m={"sm"} bg={"dark"}>
                    <Text>Number of Segments</Text>
                    <NumberInput min={1} max={100} disabled={segmentedPossible} value={numberOfSegments} onChange={(num) => {
                        if (typeof num === "string") {
                            const parsed = Number(num)
                            setNumberOfSegments(parsed)
                            return
                        }
                        setNumberOfSegments(num)

                    }}></NumberInput>
                </Box>
            </Collapse>
            )
        </Flex>
    )
}
