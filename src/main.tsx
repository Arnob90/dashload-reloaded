import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query"
import { fetchConfigFile } from "./code/defaults";
const queryClient = new QueryClient()

async function bootstrap() {
    // Configure TanStack Query to refetch config on mutations
    // The config will be fetched automatically and updated via the Tauri state channel
    // We pre-load it so it's available immediately
    try {
        await fetchConfigFile();
        // Note: Config updates from the backend are broadcast via the Tauri watch channel.
        // The frontend can subscribe to useQuery(configKey) which will refetch when needed.
    } catch (err) {
        console.error("Failed to load backend config; using fallback defaults.", err);
    }

    ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
        <React.StrictMode>
            <QueryClientProvider client={queryClient}>
                <App />
            </QueryClientProvider>
        </React.StrictMode>,
    );
}

void bootstrap();