import { Paper, Divider } from "@mantine/core";
import { useDisclosure } from "@mantine/hooks";
import { CollapseSection } from "./collapseSection";
import { HashAlgo, RequestHeaders } from "../../code/typesOfBackend";
import { RequestHeadersSection } from "./requestHeadersSection";
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
        <CollapseSection
            label="Advanced Options"
            toggleFunction={toggle}
            opened={expanded}
        >
            <Paper withBorder p="md" radius="sm" bg="dark.7">
                <RequestHeadersSection
                    requestHeaders={requestHeaders}
                    setRequestHeaders={setRequestHeaders}
                />
                <Divider color="dark.5" />
                <VerifierOptions
                    chosenAlgo={chosenAlgo}
                    setChosenAlgo={setChosenAlgo}
                    chosenHash={chosenHash}
                    setChosenHash={setChosenHash}
                />
            </Paper>
        </CollapseSection>
    );
}
