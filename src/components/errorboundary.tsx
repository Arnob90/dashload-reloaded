import React, { Component, ErrorInfo, ReactNode } from 'react';

// --- Define the props for our component ---
interface Props {
    children: ReactNode; // This will be the components that this boundary wraps
    fallback: ReactNode; // The UI to show when an error is caught
}

// --- Define the state for our component ---
interface State {
    hasError: boolean;
}

// --- The Error Boundary Class Component ---
class ErrorBoundary extends Component<Props, State> {
    // 1. Initialize the state.
    public state: State = {
        hasError: false,
    };

    // 2. This lifecycle method is called during the "render" phase after a
    // descendant component throws an error. It's used to update the state
    // so the next render will show the fallback UI.
    // It MUST be a static method.
    public static getDerivedStateFromError(_: Error): State {
        // Update state so the next render will show the fallback UI.
        return { hasError: true };
    }

    // 3. This lifecycle method is called during the "commit" phase after an
    // error has been thrown. It's the perfect place for side effects,
    // like logging the error to a service.
    public componentDidCatch(error: Error, errorInfo: ErrorInfo) {
        // You can log the error to an error reporting service like Sentry, LogRocket, etc.
        console.error("Uncaught error in component tree:", error, errorInfo);
    }

    // 4. The render method.
    public render() {
        // If an error was caught, render the fallback UI.
        if (this.state.hasError) {
            return this.props.fallback;
        }

        // Otherwise, render the children as normal.
        return this.props.children;
    }
}

export default ErrorBoundary;
