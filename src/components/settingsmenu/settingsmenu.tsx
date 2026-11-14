import { useEffect } from "react";
import { Badge, Divider, Group, Stack, Text, Title } from "@mantine/core";
import { useGetConfigFile } from "../../code/useDownloads";
import type { Config } from "../../code/typesOfBackend";

/** Formats a `{ secs, nanos }` duration as a short human-readable string. */
function formatDuration(d: { secs: number; nanos: number }): string {
    return `${d.secs}s`;
}

interface RetrySchemeViewProps {
    scheme: Config["retry_scheme"];
}

/** Read-only display of a `RetryScheme`. */
function RetrySchemeView({ scheme }: RetrySchemeViewProps) {
    return (
        <Stack gap="xs">
            <Text fw={600} size="sm">
                Retry scheme
            </Text>
            <Group gap="xs">
                <Text size="sm">max_attempts:</Text>
                <Badge>{scheme.max_attempts}</Badge>
            </Group>
            <Group gap="xs">
                <Text size="sm">base_delay:</Text>
                <Badge>{formatDuration(scheme.backoff.base_delay)}</Badge>
            </Group>
            <Group gap="xs">
                <Text size="sm">max_delay:</Text>
                <Badge>{formatDuration(scheme.backoff.max_delay)}</Badge>
            </Group>
            <Group gap="xs">
                <Text size="sm">factor:</Text>
                <Badge>{scheme.backoff.factor}</Badge>
            </Group>
            <Group gap="xs">
                <Text size="sm">jitter:</Text>
                <Badge>{scheme.backoff.jitter ? "yes" : "no"}</Badge>
            </Group>
            <Group gap="xs">
                <Text size="sm">stream_timeout:</Text>
                <Badge>{formatDuration(scheme.stream_timeout)}</Badge>
            </Group>
            <Group gap="xs">
                <Text size="sm">connection_timeout:</Text>
                <Badge>{formatDuration(scheme.connection_timeout)}</Badge>
            </Group>
        </Stack>
    );
}

interface RequestHeadersViewProps {
    headers: Config["request_headers"];
}

/** Read-only display of `RequestHeaders`. */
function RequestHeadersView({ headers }: RequestHeadersViewProps) {
    return (
        <Stack gap="xs">
            <Text fw={600} size="sm">
                Request headers
            </Text>
            <Group gap="xs">
                <Text size="sm">cookies:</Text>
                <Badge>{headers.cookies ?? "none"}</Badge>
            </Group>
            <Group gap="xs">
                <Text size="sm">referrer:</Text>
                <Badge>{headers.referrer ?? "none"}</Badge>
            </Group>
            <Group gap="xs">
                <Text size="sm">user_agent:</Text>
                <Badge>{headers.user_agent ?? "none"}</Badge>
            </Group>
        </Stack>
    );
}

/** Settings menu rendering every option of the `Config` type read-only. */
export function SettingsMenu() {
    const { mutate: getConfig, data: config, isPending } = useGetConfigFile();

    useEffect(() => {
        getConfig();
    }, [getConfig]);

    if (isPending || !config) {
        return <Text c="dimmed">Loading settings...</Text>;
    }

    return (
        <Stack gap="md">
            <Title order={3}>Settings</Title>
            <Group gap="xs">
                <Text size="sm">auto_add_download:</Text>
                <Badge>{config.auto_add_download ? "yes" : "no"}</Badge>
            </Group>
            <Group gap="xs">
                <Text size="sm">default_download_path:</Text>
                <Badge>{config.default_download_path}</Badge>
            </Group>
            <Group gap="xs">
                <Text size="sm">default_number_of_segments:</Text>
                <Badge>{config.default_number_of_segments}</Badge>
            </Group>
            <Divider />
            <RetrySchemeView scheme={config.retry_scheme} />
            <Divider />
            <RequestHeadersView headers={config.request_headers} />
        </Stack>
    );
}