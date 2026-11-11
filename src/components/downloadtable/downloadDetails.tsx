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
    Stack,
    Button,
} from "@mantine/core";
import { bytesToMIB } from "../../code/utils.ts";
import { useDownloadData, useRetryDownload } from "../../code/useDownloads.ts";
import { TableEntry } from "./tableEntry.ts";
import { Copy, Check, X, FolderOpen, Globe, RotateCcw } from "lucide-react";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { notifications } from "@mantine/notifications";
import { useState, useEffect } from "react";
import type { DownloadRunError, FailureDetails } from "../../code/typesOfBackend.ts";

interface DownloadDetailsPaneProps {
    entry: TableEntry;
    onClose: () => void;
}

// ============================================================================
// Error & Diagnostics helpers
// ============================================================================

/** Human-readable category label for a typed download error. */
function errorCategoryLabel(error: DownloadRunError): string {
    switch (error.kind) {
        case "NetworkError":
            return error.payload.status_code != null
                ? `Network Error (HTTP ${error.payload.status_code})`
                : "Network Error";
        case "ServerError":
            return error.payload.status_code != null
                ? `Server Error (HTTP ${error.payload.status_code})`
                : "Server Error";
        case "DiskError":
            return "Disk I/O Error";
        case "VerifyError":
            return "Checksum Verification Failed";
        case "TimeoutError":
            return "Download Timed Out";
        case "AlreadyRunning":
            return "Already Running";
        default:
            return "Unknown Error";
    }
}

/** Actionable remediation hint for a typed download error. */
function errorRemediation(error: DownloadRunError): string {
    switch (error.kind) {
        case "NetworkError":
            return "Check your internet connection or the remote host, then retry.";
        case "ServerError":
            return "The server rejected the request. Wait a moment and retry, or check that the URL is still valid.";
        case "DiskError":
            return "Check free disk space, folder write permissions, and that the save path still exists.";
        case "VerifyError":
            return "The downloaded file does not match its expected checksum. Retry the download to fetch a fresh copy.";
        case "TimeoutError":
            return "No data was received within the timeout window. Retry, or try again on a faster connection.";
        case "AlreadyRunning":
            return "The download is already running; no action needed.";
        default:
            return "Retry the download; if the problem persists, delete and re-add it.";
    }
}

function errorMessage(error: DownloadRunError): string {
    switch (error.kind) {
        case "NetworkError":
        case "ServerError":
        case "DiskError":
            return error.payload.message;
        case "VerifyError":
            return `Expected hash ${error.payload.expected}, got ${error.payload.actual}`;
        case "TimeoutError":
            return `Timed out waiting for next chunk after ${error.payload.secs}s`;
        case "AlreadyRunning":
            return "Download is already running";
    }
}

/** Builds a copy-ready multi-line diagnostics report for a failure. */
function buildErrorReport(entry: TableEntry, details: FailureDetails): string {
    const lines: string[] = [
        `Filename: ${entry.filename}`,
        `URL: ${entry.url}`,
        `Save location: ${entry.filepath}`,
        `Category: ${errorCategoryLabel(details.error)}`,
        `Message: ${errorMessage(details.error)}`,
    ];
    if (details.error.kind === "DiskError" && details.error.payload.path) {
        lines.push(`Path: ${details.error.payload.path}`);
    }
    if (details.error.kind === "VerifyError") {
        lines.push(`Expected hash: ${details.error.payload.expected}`);
        lines.push(`Actual hash:   ${details.error.payload.actual}`);
    }
    if (details.error.kind === "TimeoutError") {
        lines.push(`Timeout: ${details.error.payload.secs}s`);
    }
    if (details.error.kind === "NetworkError" || details.error.kind === "ServerError") {
        if (details.error.payload.status_code != null) {
            lines.push(`HTTP status: ${details.error.payload.status_code}`);
        }
    }
    lines.push(
        `Failed at: ${details.timestamp_secs > 0 ? new Date(details.timestamp_secs * 1000).toLocaleString() : "unknown"}`,
    );
    lines.push(
        `Failed segment: ${
            details.failed_segment_index != null
                ? `Segment ${details.failed_segment_index + 1}`
                : "unknown"
        }`,
    );
    lines.push(`Retry attempts made: ${details.attempts_made}`);
    lines.push(`Suggested action: ${errorRemediation(details.error)}`);
    return lines.join("\n");
}

export function DownloadDetailsPane({ entry, onClose }: DownloadDetailsPaneProps) {
    const colorMap: Record<string, string> = {
        Running: "blue",
        Completed: "teal",
        Paused: "yellow",
        Failed: "red",
        Stopped: "gray",
        Verifying: "violet",
        Retrying: "orange",
    };

    const isFailed = entry.status.kind === "Failed";
    const defaultTab = isFailed ? "error" : "overview";

    const pct =
        entry.downloadedPortion != null ? Math.round(entry.downloadedPortion * 100) : 0;

    const { data: downloadData } = useDownloadData();
    const download = downloadData?.[entry.id];
    const segments = download?.segments ?? [];
    // A download is segmented when any segment carries explicit byte boundaries.
    // If every `info` is null, the backend used a single non-ranged stream.
    const isSegmented = segments.some((segment) => segment.info != null);

    const { mutate: retryDownload } = useRetryDownload();
    const handleRetry = () => {
        retryDownload(entry.id, {
            onError: (err) => {
                notifications.show({
                    title: "Failed to Retry",
                    message: err.message,
                    color: "red",
                });
            },
            onSuccess: () => {
                notifications.show({
                    title: "Retrying",
                    message: "The download has been restarted.",
                    color: "blue",
                });
            },
        });
    };

    // Keep the active tab in sync with the download's lifecycle: when a
    // download fails, open the Error & Diagnostics tab; if it recovers while
    // that tab is active, fall back to Overview so the pane never renders
    // blank (the "error" panel is unmounted as soon as `isFailed` flips off).
    const [activeTab, setActiveTab] = useState<string | null>(null);
    const currentTab = activeTab ?? defaultTab;
    useEffect(() => {
        setActiveTab((prev) => {
            const tab = prev ?? defaultTab;
            if (isFailed) {
                return tab === "error" ? tab : "error";
            }
            return tab === "error" ? "overview" : tab;
        });
    }, [isFailed]);

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
            <Tabs value={currentTab} onChange={setActiveTab} variant="pills" radius="sm">
                {/* Top Header: Tabs on Left, Close (X) on Right */}
                <Group justify="space-between" mb="sm">
                    <Tabs.List>
                        <Tabs.Tab value="overview" fz="xs" fw={500}>
                            Overview
                        </Tabs.Tab>
                        {isFailed && (
                            <Tabs.Tab value="error" fz="xs" fw={500}>
                                Error & Diagnostics
                            </Tabs.Tab>
                        )}
                        <Tabs.Tab value="segments" fz="xs" fw={500}>
                            Segments
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

                {/* TAB 1b: ERROR & DIAGNOSTICS (only visible when Failed) */}
                {isFailed && (
                    <Tabs.Panel value="error">
                        <Stack gap="sm">
                            <Paper withBorder p="xs" radius="sm" bg="dark.8">
                                <Text size="xs" fw={600} c="dimmed" mb={2}>
                                    FAILURE CATEGORY
                                </Text>
                                <Badge color="red" variant="light" size="sm">
                                    {errorCategoryLabel(entry.failureDetails!.error)}
                                </Badge>
                            </Paper>

                            <Paper withBorder p="xs" radius="sm" bg="dark.8">
                                <Text size="xs" fw={600} c="dimmed" mb={2}>
                                    FAILURE MESSAGE
                                </Text>
                                <Text size="xs" style={{ wordBreak: "break-word" }}>
                                    {errorMessage(entry.failureDetails!.error)}
                                </Text>
                            </Paper>

                            {entry.failureDetails!.error.kind === "DiskError" &&
                                entry.failureDetails!.error.payload.path && (
                                    <Paper withBorder p="xs" radius="sm" bg="dark.8">
                                        <Text size="xs" fw={600} c="dimmed" mb={2}>
                                            AFFECTED PATH
                                        </Text>
                                        <Text size="xs" style={{ wordBreak: "break-all" }}>
                                            {entry.failureDetails!.error.payload.path}
                                        </Text>
                                    </Paper>
                                )}

                            {entry.failureDetails!.error.kind === "VerifyError" && (
                                <Paper withBorder p="xs" radius="sm" bg="dark.8">
                                    <Text size="xs" fw={600} c="dimmed" mb={2}>
                                        CHECKSUM MISMATCH
                                    </Text>
                                    <Text size="xs" style={{ wordBreak: "break-all" }}>
                                        Expected: {entry.failureDetails!.error.payload.expected}
                                    </Text>
                                    <Text size="xs" style={{ wordBreak: "break-all" }}>
                                        Actual:   {entry.failureDetails!.error.payload.actual}
                                    </Text>
                                </Paper>
                            )}

                            <Paper withBorder p="xs" radius="sm" bg="dark.8">
                                <Text size="xs" fw={600} c="dimmed" mb={2}>
                                    SUGGESTED ACTION
                                </Text>
                                <Text size="xs" c="dimmed">
                                    {errorRemediation(entry.failureDetails!.error)}
                                </Text>
                            </Paper>

                            <SimpleGrid cols={{ base: 2, sm: 3 }} spacing="xs">
                                <Paper withBorder p="xs" radius="sm" bg="dark.8">
                                    <Text size="xs" fw={600} c="dimmed" mb={2}>
                                        FAILED AT
                                    </Text>
                                    <Text size="xs">
                                        {entry.failureDetails!.timestamp_secs > 0
                                            ? new Date(
                                                entry.failureDetails!.timestamp_secs * 1000,
                                            ).toLocaleString()
                                            : "unknown"}
                                    </Text>
                                </Paper>
                                <Paper withBorder p="xs" radius="sm" bg="dark.8">
                                    <Text size="xs" fw={600} c="dimmed" mb={2}>
                                        FAILED SEGMENT
                                    </Text>
                                    <Text size="xs">
                                        {entry.failureDetails!.failed_segment_index != null
                                            ? `Segment ${entry.failureDetails!.failed_segment_index + 1}`
                                            : "unknown"}
                                    </Text>
                                </Paper>
                                <Paper withBorder p="xs" radius="sm" bg="dark.8">
                                    <Text size="xs" fw={600} c="dimmed" mb={2}>
                                        ATTEMPTS MADE
                                    </Text>
                                    <Text size="xs">{entry.failureDetails!.attempts_made}</Text>
                                </Paper>
                            </SimpleGrid>

                            <Group justify="space-between" mt="xs">
                                <CopyButton
                                    value={buildErrorReport(entry, entry.failureDetails!)}
                                    timeout={3000}
                                >
                                    {({ copied, copy }) => (
                                        <Button
                                            variant="light"
                                            size="xs"
                                            leftSection={copied ? <Check size={12} /> : <Copy size={12} />}
                                            onClick={copy}
                                        >
                                            {copied ? "Copied" : "Copy Error Details"}
                                        </Button>
                                    )}
                                </CopyButton>
                                <Button
                                    variant="filled"
                                    color="red"
                                    size="xs"
                                    leftSection={<RotateCcw size={12} />}
                                    onClick={handleRetry}
                                >
                                    Retry Download
                                </Button>
                            </Group>
                        </Stack>
                    </Tabs.Panel>
                )}

                {/* TAB 2: PER-SEGMENT PROGRESS */}
                <Tabs.Panel value="segments">
                    <ScrollArea h={160} offsetScrollbars>
                        {segments.length === 0 ? (
                            <Text size="xs" c="dimmed">
                                No segment data available.
                            </Text>
                        ) : !isSegmented ? (
                            <Box>
                                <Text size="xs" c="dimmed" mb={6}>
                                    This download is not segmented.
                                </Text>
                                <Progress
                                    value={pct}
                                    size="md"
                                    radius="xl"
                                    color={pct === 100 ? "teal" : "blue"}
                                    mb={6}
                                    styles={{
                                        section: {
                                            transition: "width 300ms ease-in-out",
                                        },
                                    }}
                                />
                                <Group justify="space-between">
                                    <Text size="xs" c="dimmed">
                                        {pct}% • {bytesToMIB(entry.downloaded).toFixed(1)} MiB of{" "}
                                        {entry.total != null
                                            ? `${bytesToMIB(entry.total).toFixed(1)} MiB`
                                            : "Unknown"}
                                    </Text>
                                </Group>
                            </Box>
                        ) : (
                            <Stack gap="xs">
                                {segments.map((segment, index) => {
                                    const size = segment.info
                                        ? segment.info.end - segment.info.start + 1
                                        : null;
                                    const downloaded =
                                        size != null
                                            ? Math.min(segment.last_downloaded, size)
                                            : segment.last_downloaded;
                                    const segmentPct =
                                        size != null && size > 0
                                            ? Math.round((downloaded / size) * 100)
                                            : 0;

                                    return (
                                        <Box key={index}>
                                            <Group justify="space-between" mb={2}>
                                                <Text size="xs" fw={500}>
                                                    Segment {index + 1}
                                                    {segment.info && (
                                                        <Text span size="xs" c="dimmed" ml={6}>
                                                            ({segment.info.start}-{segment.info.end})
                                                        </Text>
                                                    )}
                                                </Text>
                                                <Text size="xs" c="dimmed">
                                                    {segmentPct}% •{" "}
                                                    {bytesToMIB(downloaded).toFixed(1)}
                                                    {size != null
                                                        ? ` / ${bytesToMIB(size).toFixed(1)} MiB`
                                                        : " MiB"}
                                                </Text>
                                            </Group>
                                            <Progress
                                                value={segmentPct}
                                                size="sm"
                                                radius="xl"
                                                color={segmentPct === 100 ? "teal" : "blue"}
                                                styles={{
                                                    section: {
                                                        transition: "width 300ms ease-in-out",
                                                    },
                                                }}
                                            />
                                        </Box>
                                    );
                                })}
                            </Stack>
                        )}
                    </ScrollArea>
                </Tabs.Panel>

                {/* TAB 3: HTTP HEADERS */}
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
