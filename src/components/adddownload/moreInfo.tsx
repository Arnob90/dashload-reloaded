import { Collapse, Text, Flex, Paper, NavLink, Group, Divider } from "@mantine/core";
import { useDisclosure } from "@mantine/hooks";
import { Info } from "lucide-react";

export interface MoreInfoProps {
    url: string;
    filename: string | null;
    filesize: string | null;
    segmentedPossible: boolean;
}

export function MoreInfo({ filename, filesize, segmentedPossible }: MoreInfoProps) {
    const [expanded, { toggle }] = useDisclosure(false);

    return (
        <Flex direction="column" gap="xs">
            {/* Labeled Collapsible Header Button */}
            {/* Mantine NavLink handles left icon, right chevron, and 100% width automatically */}
            <NavLink
                label="File Details"
                leftSection={<Info size={16} />}
                onClick={toggle}
                opened={expanded}
                variant="light"
                style={{ borderRadius: 6 }}
                px="0"
            />

            {/* Styled Details Card */}
            <Collapse expanded={expanded}>
                <Paper withBorder p="sm" radius="sm" bg="dark.7">
                    <Flex direction="column" gap="xs">
                        <Group justify="space-between">
                            <Text size="xs" c="dimmed">Source File:</Text>
                            <Text size="xs" fw={500}>{filename ?? "Unknown"}</Text>
                        </Group>
                        <Divider />
                        <Group justify="space-between">
                            <Text size="xs" c="dimmed">Total Size:</Text>
                            <Text size="xs" fw={500}>{filesize ?? "Unknown (Streamed)"}</Text>
                        </Group>
                        <Divider />
                        <Group justify="space-between">
                            <Text size="xs" c="dimmed">Multi-segment Support:</Text>
                            <Text size="xs" fw={500} c={segmentedPossible ? "teal" : "yellow"}>
                                {segmentedPossible ? "Supported" : "Single Stream Only"}
                            </Text>
                        </Group>
                    </Flex>
                </Paper>
            </Collapse>
        </Flex>
    );
}
