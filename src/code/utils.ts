export function bytesToMIB(bytes: number): number {
    return Number((bytes / 1024 / 1024).toFixed(2))
}

/**
 * Synchronously joins a directory path and a filename cross-platform.
 * Handles Windows (\), Linux/macOS (/), and trailing/leading slashes.
 */
export function joinPaths(dir: string, filename: string): string {
    // 1. Remove trailing slashes from the directory
    const cleanDir = dir.trim().replace(/[/\\]+$/, "");

    // 2. Remove leading slashes from the filename
    const cleanFile = filename.trim().replace(/^[/\\]+/, "");

    // 3. Preserve Windows backslash if the directory used backslashes, otherwise use forward slash
    const separator = cleanDir.includes("\\") ? "\\" : "/";

    return `${cleanDir}${separator}${cleanFile}`;
}
