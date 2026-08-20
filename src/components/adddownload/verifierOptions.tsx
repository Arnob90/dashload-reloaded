import { Collapse, Flex, Paper, NavLink, Divider, TextInput, Select } from "@mantine/core";
import { useDisclosure } from "@mantine/hooks";
import { Info } from "lucide-react";
import { HashAlgo } from "../../code/typesOfBackend";
interface VerifierProps {
    chosenAlgo: HashAlgo | null,
    setChosenAlgo?: (value: HashAlgo | null) => void;
    chosenHash: string,
    setChosenHash?: (hash: string) => void;
}
export function VerifierOptions({ chosenAlgo, setChosenAlgo, chosenHash, setChosenHash }: VerifierProps) {
    const [expanded, { toggle }] = useDisclosure(false);

    return (
        <Flex direction="column" gap="xs">
            {/* Labeled Collapsible Header Button */}
            {/* Mantine NavLink handles left icon, right chevron, and 100% width automatically */}
            <NavLink
                label="Checksums"
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
                        <Select label="Checksum algorithms" data={["Sha256", "Sha512", "Md5"]} onChange={(val) => setChosenAlgo?.(val)} placeholder="Pick algorithm to verify with">
                        </Select>
                        <Divider />
                        <TextInput label="Hash checksum to match against" placeholder="Enter hash here" disabled={chosenAlgo === null} value={chosenHash} onChange={(event) => setChosenHash?.(event.target.value)}>
                        </TextInput>
                    </Flex>
                </Paper>
            </Collapse>
        </Flex>
    );
}
