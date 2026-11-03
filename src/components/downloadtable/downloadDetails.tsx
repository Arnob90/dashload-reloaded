import {
    Paper,
    Tabs,
    Text,
    Group,
    Badge,
    Progress,
    ActionIcon,
    SimpleGrid,
    Box,
    CopyButton,
    Tooltip,
    Code,
    ScrollArea,
} from "@mantine/core";
import { bytesToMIB } from "../../code/utils.ts";
import { TableEntry } from "./tableEntry.ts";
import { Copy, Check, X, FolderOpen, Globe } from "lucide-react";
import { revealItemInDir } from "@tauri-apps/plugin-opener";

interface DownloadDetailsPaneProps {
    entry: TableEntry;
    onClose: () => void;
}

export function DownloadDetailsPane({ entry, onClose }: DownloadDetailsPaneProps) {
    const colorMap: Record<string, string> = {
        Running: "blue",
        Completed: "teal",
        Paused: "yellow",
        Failed: "red",
        Stopped: "gray",
        Verifying: "violet",
    };

    const pct =
        entry.downloadedPortion != null ? Math.round(entry.downloadedPortion * 100) : 0;

    const handleReveal = async () => {
        try {
            await revealItemInDir(entry.filepath);
        } catch (e) {
            console.error(e);
        }
    };

    return (
        <Paper
            withBorder
            h={260} // 👈 Increased height for comfortable breathing room
            bg="dark.7" // 👈 Distinct dark-panel elevation
            radius={0}
            style={{
                borderTop: "1px solid var(--mantine-color-dark-4)",
                borderLeft: "none",
                borderRight: "none",
                borderBottom: "none",
            }}
            p="md"
        >
            <Tabs defaultValue="overview" variant="pills" radius="sm">
                {/* Top Header: Tabs on Left, Close (X) on Right */}
                <Group justify="space-between" mb="sm">
                    <Tabs.List>
                        <Tabs.Tab value="overview" fz="xs" fw={500}>
                            Overview
                        </Tabs.Tab>
                        <Tabs.Tab value="headers" fz="xs" fw={500}>
                            HTTP Context
                        </Tabs.Tab>
                    </Tabs.List>

                    <ActionIcon
                        variant="subtle"
                        color="gray"
                        size="sm"
                        onClick={onClose}
                        title="Close details (Esc)"
                    >
                        <X size={16} />
                    </ActionIcon>
                </Group>

                {/* TAB 1: OVERVIEW */}
                <Tabs.Panel value="overview">
                    {/* File Title and Status Badge */}
                    <Group justify="space-between" mb="xs">
                        <Text size="sm" fw={600} truncate maw="80%">
                            {entry.filename}
                        </Text>
                        <Badge color={colorMap[entry.status.kind] ?? "gray"} variant="light" size="sm">
                            {entry.status.kind}
                        </Badge>
                    </Group>

                    {/* Progress Bar & Stats */}
                    <Box mb="md">
                        <Progress
                            value={pct}
                            size="md" // 8px thick instead of thin wire
                            radius="xl"
                            color={pct === 100 ? "teal" : "blue"}
                            mb={6}
                            styles={{
                                section: {
                                    transition: "width 300ms ease-in-out", // Smooth transition for progress bar
                                }
                            }}
                        />
                        <Group justify="space-between">
                            <Text size="xs" c="dimmed">
                                {pct}% • {bytesToMIB(entry.downloaded).toFixed(1)} MiB of{" "}
                                {entry.total != null ? `${bytesToMIB(entry.total).toFixed(1)} MiB` : "Unknown"}
                            </Text>
                            <Text size="xs" fw={500} c={entry.speed ? "blue" : "dimmed"}>
                                {entry.speed && entry.speed > 0
                                    ? `${bytesToMIB(entry.speed).toFixed(2)} MiB/s`
                                    : "-"}
                            </Text>
                        </Group>
                    </Box>

                    {/* Structured 2-Column Cards for URL & Filepath */}
                    <SimpleGrid cols={{ base: 1, sm: 2 }} spacing="sm">
                        {/* Download URL Box */}
                        <Paper withBorder p="xs" radius="sm" bg="dark.8">
                            <Text size="xs" fw={600} c="dimmed" mb={4}>
                                DOWNLOAD URL
                            </Text>
                            <Group justify="space-between" wrap="nowrap">
                                <Group gap="xs" wrap="nowrap" style={{ flex: 1, overflow: "hidden" }}>
                                    <Globe size={14} className="text-dimmed" />
                                    <Text size="xs" truncate title={entry.url}>
                                        {entry.url}
                                    </Text>
                                </Group>
                                <CopyButton value={entry.url} timeout={2000}>
                                    {({ copied, copy }) => (
                                        <Tooltip label={copied ? "Copied" : "Copy URL"} position="top">
                                            <ActionIcon variant="subtle" color={copied ? "teal" : "gray"} size="xs" onClick={copy}>
                                                {copied ? <Check size={12} /> : <Copy size={12} />}
                                            </ActionIcon>
                                        </Tooltip>
                                    )}
                                </CopyButton>
                            </Group>
                        </Paper>

                        {/* Save Location Box */}
                        <Paper withBorder p="xs" radius="sm" bg="dark.8">
                            <Text size="xs" fw={600} c="dimmed" mb={4}>
                                SAVE LOCATION
                            </Text>
                            <Group justify="space-between" wrap="nowrap">
                                <Group gap="xs" wrap="nowrap" style={{ flex: 1, overflow: "hidden" }}>
                                    <FolderOpen size={14} className="text-dimmed" />
                                    <Text size="xs" truncate title={entry.filepath}>
                                        {entry.filepath}
                                    </Text>
                                </Group>
                                <Tooltip label="Show in File Manager" position="top">
                                    <ActionIcon variant="subtle" color="gray" size="xs" onClick={handleReveal}>
                                        <FolderOpen size={12} />
                                    </ActionIcon>
                                </Tooltip>
                            </Group>
                        </Paper>
                    </SimpleGrid>
                </Tabs.Panel>

                {/* TAB 2: HTTP HEADERS */}
                <Tabs.Panel value="headers">
                    <ScrollArea h={160} offsetScrollbars>
                        <SimpleGrid cols={1} spacing="xs">
                            <Box>
                                <Text size="xs" fw={600} c="dimmed" mb={2}>
                                    USER-AGENT:
                                </Text>
                                <Code block fz="xs">
                                    {entry.headers?.userAgent ?? "Default (Mozilla/5.0...)"}
                                </Code>
                            </Box>

                            <Box>
                                <Text size="xs" fw={600} c="dimmed" mb={2}>
                                    REFERRER:
                                </Text>
                                <Code block fz="xs">
                                    {entry.headers?.referrer ?? "None"}
                                </Code>
                            </Box>

                            <Box>
                                <Text size="xs" fw={600} c="dimmed" mb={2}>
                                    SESSION COOKIES:
                                </Text>
                                <Code block fz="xs" style={{ wordBreak: "break-all" }}>
                                    {entry.headers?.cookies ?? "None"}
                                </Code>
                            </Box>
                        </SimpleGrid>
                    </ScrollArea>
                </Tabs.Panel>
            </Tabs>
        </Paper>
    );
}
