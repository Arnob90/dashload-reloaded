import { Box, Flex, Text, TextInput } from "@mantine/core";
import { RequestHeaders } from "../../code/typesOfBackend";
export interface RequestHeadersSectionProps {
    requestHeaders: RequestHeaders;
    setRequestHeaders: (headers: RequestHeaders) => void;
}

export function RequestHeadersSection({ requestHeaders, setRequestHeaders }: RequestHeadersSectionProps) {

    return (
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
                            user_agent: e.target.value || null // null prevents sending empty strings
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
        </Flex>
    )

}

