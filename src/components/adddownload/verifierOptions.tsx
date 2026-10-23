import { Group, TextInput, Select, Text, Box } from "@mantine/core";
import { HashAlgo } from "../../code/typesOfBackend";

interface VerifierProps {
    chosenAlgo: HashAlgo | null;
    setChosenAlgo: (value: HashAlgo | null) => void;
    chosenHash: string;
    setChosenHash: (hash: string) => void;
}

export function VerifierOptions({ chosenAlgo, setChosenAlgo, chosenHash, setChosenHash }: VerifierProps) {
    return (
        <Box>
            <Text size="xs" fw={600} c="dimmed" mb="xs" tt="uppercase">
                File Verification
            </Text>
            {/* wrap="nowrap" prevents them from stacking; flex: 1 makes the input stretch */}
            <Group align="flex-start" wrap="nowrap" gap="sm">
                <Select
                    label="Algorithm"
                    data={[
                        { value: "Sha256", label: "SHA-256" },
                        { value: "Sha512", label: "SHA-512" },
                        { value: "Md5", label: "MD5" }
                    ]}
                    value={chosenAlgo}
                    onChange={(val) => setChosenAlgo(val as HashAlgo | null)}
                    placeholder="None"
                    clearable // 👈 Allows the user to easily click 'X' to remove hashing
                    w={130}
                />

                <TextInput
                    style={{ flex: 1 }}
                    label="Expected Checksum"
                    placeholder={chosenAlgo ? "Paste hash string here..." : "Select algorithm first"}
                    disabled={!chosenAlgo}
                    value={chosenHash}
                    onChange={(event) => setChosenHash(event.target.value)}
                />
            </Group>
        </Box>
    );
}
