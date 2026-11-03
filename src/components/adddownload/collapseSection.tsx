import { Collapse, Flex, NavLink, Paper, Text } from "@mantine/core";
import { SlidersHorizontal, ChevronRight, ChevronDown } from "lucide-react";
export interface CollapseSectionProps {
    label: string;
    children?: React.ReactNode;
    toggleFunction: () => void;
    opened: boolean;
}
export function CollapseSection({ label, children, toggleFunction, opened }: CollapseSectionProps) {
    return (
        <Flex direction="column" gap="xs">
            <NavLink
                label={<Text fw={500}>{label}</Text>}
                leftSection={<SlidersHorizontal size={16} />}
                rightSection={opened ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
                onClick={toggleFunction}
                opened={opened}
                variant="subtle"
                px={0}
                py={4}
                style={{ borderRadius: 6 }}
            />
            <Collapse expanded={opened}>
                <Paper withBorder p="md" radius="sm" bg="dark.7">
                    {children}
                </Paper>
            </Collapse>
        </Flex>
    )
}

