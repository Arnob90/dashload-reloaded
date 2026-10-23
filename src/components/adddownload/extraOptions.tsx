import { Collapse, ActionIcon, Text, Flex, Box } from "@mantine/core";
import { ChevronDown, ChevronRight } from "lucide-react";
import { useDisclosure } from "@mantine/hooks"; // Let the component manage its own state!

// --- A more descriptive name and clearer props ---
export interface CollapsibleSectionProps {
    children: React.ReactNode;
    optionText: string;
    initialState?: boolean; // Allow parent to control the initial state
}

export function CollapsibleSection({
    children,
    optionText,
    initialState = false, // Default to closed
}: CollapsibleSectionProps) {

    // 1. The component now manages its own open/closed state.
    // This is a much better encapsulation.
    const [isOpen, { toggle }] = useDisclosure(initialState);

    return (
        // Use a Box or simple div as the main container.
        <Box>
            {/* --- 2. The Trigger is now first, which matches the visual layout --- */}
            <Flex
                onClick={toggle} // The whole Flex area is now clickable
                align="center"
                gap="0.2vw"
                style={{ cursor: 'pointer', userSelect: 'none' }} // Good UX for clickable items
            >
                <ActionIcon variant="transparent" color="gray.6" size="sm">
                    {/* 3. The icon logic is now correct: Down for open, Right for closed */}
                    {isOpen ? <ChevronDown size={16} /> : <ChevronRight size={16} />}
                </ActionIcon>
                <Text c="dimmed" size="sm" mt={2}>
                    {/* 4. The text correctly reflects the state */}
                    {isOpen ? `Hide ${optionText}` : `Show ${optionText}`}
                </Text>
            </Flex>

            {/* --- 5. The Content is now a sibling, placed after the trigger --- */}
            <Collapse expanded={isOpen}>
                {/* It's good practice to add some margin to the content */}
                <Box mt="sm" ml="lg">
                    {children}
                </Box>
            </Collapse>
        </Box>
    );
}
