import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query"
import { initConfigFile } from "./code/defaults";
const queryClient = new QueryClient()

async function bootstrap() {
    try {
        await initConfigFile();
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
