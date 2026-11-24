import { useEffect, useState } from "react";
import {
    Modal,
    Switch,
    TextInput,
    NumberInput,
    Button,
    ActionIcon,
    Tabs,
    Flex,
    Group,
    Box,
    Text,
    Divider,
    LoadingOverlay,
    Paper,
} from "@mantine/core";
import { notifications } from "@mantine/notifications";
import { Folder, Settings, Globe, RefreshCw } from "lucide-react";
import { useConfig, useSetConfig, useOpenFile } from "../../code/useDownloads";
import type { Config } from "../../code/typesOfBackend";

export interface ConfigMenuProps {
    opened: boolean;
    onClose: () => void;
}

export function ConfigMenu({ opened, onClose }: ConfigMenuProps) {
    const { data: configData, isPending: isLoadingConfig } = useConfig();
    const { mutate: saveConfig, isPending: isSaving } = useSetConfig();
    const openFileMutation = useOpenFile();

    const [draft, setDraft] = useState<Config | null>(null);

    // Sync local draft whenever modal opens or backend config refreshes
    useEffect(() => {
        if (opened && configData) {
            setDraft(structuredClone(configData));
        }
    }, [opened, configData]);

    const handleBrowseFolder = async () => {
        try {
            const folderPath = await openFileMutation.mutateAsync();
            if (folderPath && draft) {
                setDraft({ ...draft, default_download_path: folderPath });
            }
        } catch {
            notifications.show({
                title: "Folder Selection",
                message: "No folder was selected",
                color: "yellow",
            });
        }
    };

    const handleSave = (e: React.SubmitEvent<HTMLFormElement>) => {
        e.preventDefault();
        if (!draft) return;

        saveConfig(draft, {
            onSuccess: () => {
                notifications.show({
                    title: "Settings Saved",
                    message: "Your configuration has been updated.",
                    color: "teal",
                });
                onClose();
            },
            onError: (err) => {
                notifications.show({
                    title: "Failed to Save Settings",
                    message: err.message,
                    color: "red",
                });
            },
        });
    };

    return (
        <Modal
            opened={opened}
            onClose={onClose}
            title={
                <Group gap="xs">
                    <Settings size={18} />
                    <Text fw={600}>Settings</Text>
                </Group>
            }
            size="lg"
            centered
        >
            <Box pos="relative" mih={280}>
                <LoadingOverlay
                    visible={isLoadingConfig || isSaving || openFileMutation.isPending}
                    zIndex={1000}
                    overlayProps={{ radius: "sm", blur: 2 }}
                />

                {draft && (
                    <form onSubmit={handleSave}>
                        <Tabs defaultValue="general" variant="pills" radius="sm">
                            <Tabs.List mb="md">
                                <Tabs.Tab value="general" leftSection={<Settings size={14} />}>
                                    General
                                </Tabs.Tab>
                                <Tabs.Tab value="network" leftSection={<Globe size={14} />}>
                                    Default Headers
                                </Tabs.Tab>
                                <Tabs.Tab value="retries" leftSection={<RefreshCw size={14} />}>
                                    Retries
                                </Tabs.Tab>
                            </Tabs.List>

                            {/* TAB 1: GENERAL */}
                            <Tabs.Panel value="general">
                                <Flex direction="column" gap="md">
                                    <Paper withBorder p="sm" radius="sm" bg="dark.7">
                                        <Group justify="space-between" wrap="nowrap">
                                            <Box>
                                                <Text size="sm" fw={500}>
                                                    Auto-Start Intercepted Downloads
                                                </Text>
                                                <Text size="xs" c="dimmed">
                                                    Skip the confirmation modal when links arrive from the browser extension
                                                </Text>
                                            </Box>
                                            <Switch
                                                checked={draft.auto_add_download}
                                                onChange={(e) =>
                                                    setDraft({
                                                        ...draft,
                                                        auto_add_download: e.currentTarget.checked,
                                                    })
                                                }
                                            />
                                        </Group>
                                    </Paper>

                                    <TextInput
                                        label="Default Download Directory"
                                        description="Used for auto-started downloads and as the starting folder in dialogs"
                                        placeholder="/path/to/downloads"
                                        value={draft.default_download_path}
                                        onChange={(e) =>
                                            setDraft({
                                                ...draft,
                                                default_download_path: e.currentTarget.value,
                                            })
                                        }
                                        rightSection={
                                            <ActionIcon
                                                variant="subtle"
                                                color="gray"
                                                onClick={handleBrowseFolder}
                                                title="Browse folder"
                                            >
                                                <Folder size={18} />
                                            </ActionIcon>
                                        }
                                        required
                                    />

                                    <Group grow align="flex-start">
                                        <NumberInput
                                            label="Default Segments"
                                            description="Concurrent connections per file"
                                            min={1}
                                            max={32}
                                            value={draft.default_number_of_segments}
                                            onChange={(val) =>
                                                setDraft({
                                                    ...draft,
                                                    default_number_of_segments:
                                                        typeof val === "number" ? val : 1,
                                                })
                                            }
                                        />

                                        <NumberInput
                                            label="UI Polling Rate (ms)"
                                            description="Lower is smoother; higher saves CPU"
                                            min={50}
                                            max={5000}
                                            step={50}
                                            value={draft.polling_rate_ms}
                                            onChange={(val) =>
                                                setDraft({
                                                    ...draft,
                                                    polling_rate_ms:
                                                        typeof val === "number" ? val : 300,
                                                })
                                            }
                                        />
                                    </Group>
                                </Flex>
                            </Tabs.Panel>

                            {/* TAB 2: DEFAULT HEADERS */}
                            <Tabs.Panel value="network">
                                <Paper withBorder p="md" radius="sm" bg="dark.7">
                                    <Flex direction="column" gap="sm">
                                        <Text size="xs" fw={600} c="dimmed" tt="uppercase">
                                            Fallback HTTP Headers
                                        </Text>

                                        <TextInput
                                            label="Default User-Agent"
                                            placeholder="Mozilla/5.0..."
                                            value={draft.request_headers.user_agent ?? ""}
                                            onChange={(e) =>
                                                setDraft({
                                                    ...draft,
                                                    request_headers: {
                                                        ...draft.request_headers,
                                                        user_agent: e.currentTarget.value || null,
                                                    },
                                                })
                                            }
                                        />

                                        <TextInput
                                            label="Default Referrer"
                                            placeholder="https://..."
                                            value={draft.request_headers.referrer ?? ""}
                                            onChange={(e) =>
                                                setDraft({
                                                    ...draft,
                                                    request_headers: {
                                                        ...draft.request_headers,
                                                        referrer: e.currentTarget.value || null,
                                                    },
                                                })
                                            }
                                        />

                                        <TextInput
                                            label="Default Cookies"
                                            placeholder="session_id=..."
                                            value={draft.request_headers.cookies ?? ""}
                                            onChange={(e) =>
                                                setDraft({
                                                    ...draft,
                                                    request_headers: {
                                                        ...draft.request_headers,
                                                        cookies: e.currentTarget.value || null,
                                                    },
                                                })
                                            }
                                        />
                                    </Flex>
                                </Paper>
                            </Tabs.Panel>

                            {/* TAB 3: RETRIES */}
                            <Tabs.Panel value="retries">
                                <Flex direction="column" gap="md">
                                    <Group grow align="flex-start">
                                        <NumberInput
                                            label="Max Retry Attempts"
                                            description="Retries after the initial attempt fails"
                                            min={0}
                                            max={20}
                                            value={draft.retry_scheme.max_attempts}
                                            onChange={(val) =>
                                                setDraft({
                                                    ...draft,
                                                    retry_scheme: {
                                                        ...draft.retry_scheme,
                                                        max_attempts: typeof val === "number" ? val : 0,
                                                    },
                                                })
                                            }
                                        />

                                        <NumberInput
                                            label="Backoff Factor"
                                            description="Multiplier between consecutive retries"
                                            min={1}
                                            max={5}
                                            step={0.5}
                                            decimalScale={1}
                                            value={draft.retry_scheme.backoff.factor}
                                            onChange={(val) =>
                                                setDraft({
                                                    ...draft,
                                                    retry_scheme: {
                                                        ...draft.retry_scheme,
                                                        backoff: {
                                                            ...draft.retry_scheme.backoff,
                                                            factor: typeof val === "number" ? val : 2.0,
                                                        },
                                                    },
                                                })
                                            }
                                        />
                                    </Group>

                                    <Paper withBorder p="sm" radius="sm" bg="dark.7">
                                        <Group justify="space-between" wrap="nowrap">
                                            <Box>
                                                <Text size="sm" fw={500}>
                                                    Enable Retry Jitter
                                                </Text>
                                                <Text size="xs" c="dimmed">
                                                    Adds random variance to backoff delays so segments don't hammer servers in sync
                                                </Text>
                                            </Box>
                                            <Switch
                                                checked={draft.retry_scheme.backoff.jitter}
                                                onChange={(e) =>
                                                    setDraft({
                                                        ...draft,
                                                        retry_scheme: {
                                                            ...draft.retry_scheme,
                                                            backoff: {
                                                                ...draft.retry_scheme.backoff,
                                                                jitter: e.currentTarget.checked,
                                                            },
                                                        },
                                                    })
                                                }
                                            />
                                        </Group>
                                    </Paper>
                                </Flex>
                            </Tabs.Panel>
                        </Tabs>

                        <Divider my="md" color="dark.5" />

                        <Group justify="flex-end" gap="sm">
                            <Button variant="subtle" color="gray" onClick={onClose}>
                                Cancel
                            </Button>
                            <Button type="submit" loading={isSaving}>
                                Save Settings
                            </Button>
                        </Group>
                    </form>
                )}
            </Box>
        </Modal>
    );
}
