import { Collapse, Flex, TextInput, NavLink, Paper, Text, Divider, Box } from "@mantine/core";
import { useDisclosure } from "@mantine/hooks";
import { SlidersHorizontal, ChevronDown, ChevronRight } from "lucide-react";
import { HashAlgo, RequestHeaders } from "../../code/typesOfBackend";
import { VerifierOptions } from "./verifierOptions";

export interface AdvancedOptionsProps {
    requestHeaders: RequestHeaders;
    setRequestHeaders: (headers: RequestHeaders) => void;
    chosenAlgo: HashAlgo | null;
    setChosenAlgo: (value: HashAlgo | null) => void;
    chosenHash: string;
    setChosenHash: (hash: string) => void;
}

export function AdvancedOptions({
    requestHeaders,
    setRequestHeaders,
    chosenAlgo,
    setChosenAlgo,
    chosenHash,
    setChosenHash
}: AdvancedOptionsProps) {
    const [expanded, { toggle }] = useDisclosure(false);

    return (
        <Flex direction="column" gap="xs">
            {/* Toggle Button */}
            <NavLink
                label={<Text fw={500}>Advanced Options</Text>}
                leftSection={<SlidersHorizontal size={16} />}
                rightSection={expanded ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
                onClick={toggle}
                opened={expanded}
                variant="subtle"
                px={0}
                py={4}
                style={{ borderRadius: 6 }}
            />

            <Collapse expanded={expanded}>
                <Paper withBorder p="md" radius="sm" bg="dark.7">
                    <Flex direction="column" gap="md">

                        {/* Section 1: HTTP Headers */}
                        <Box>
                            <Text size="xs" fw={600} c="dimmed" mb="xs" tt="uppercase">
                                HTTP Headers
                            </Text>
                            <Flex direction="column" gap="sm">
                                <TextInput
                                    label="User Agent"
                                    value={requestHeaders.user_agent ?? ""}
                                    onChange={(e) => setRequestHeaders({
                                        ...requestHeaders,
                                        user_agent: e.target.value || null // 👈 || null prevents sending empty strings
                                    })}
                                    placeholder="Mozilla/5.0..."
                                />
                                <TextInput
                                    label="Referrer"
                                    value={requestHeaders.referrer ?? ""}
                                    onChange={(e) => setRequestHeaders({
                                        ...requestHeaders,
                                        referrer: e.target.value || null
                                    })}
                                    placeholder="https://..."
                                />
                                <TextInput
                                    label="Cookies"
                                    value={requestHeaders.cookies ?? ""}
                                    onChange={(e) => setRequestHeaders({
                                        ...requestHeaders,
                                        cookies: e.target.value || null
                                    })}
                                    placeholder="session_id=123456789..."
                                />
                            </Flex>
                        </Box>

                        <Divider color="dark.5" />

                        {/* Section 2: File Verification */}
                        <VerifierOptions
                            chosenAlgo={chosenAlgo}
                            setChosenAlgo={setChosenAlgo}
                            chosenHash={chosenHash}
                            setChosenHash={setChosenHash}
                        />

                    </Flex>
                </Paper>
            </Collapse>
        </Flex>
    );
}
