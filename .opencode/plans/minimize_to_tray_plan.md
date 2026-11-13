# Implementation Plan: Minimize to Taskbar (System Tray)

## Overview

Implement the "close to tray" feature so that clicking the window's X button hides the window into the system tray instead of terminating the app. A tray icon with a right-click context menu must allow the user to re-show the window or quit the application.

## Current State

| Component | Location | Behavior |
|-----------|----------|----------|
| Rust window event | `src-tauri/src/lib.rs` (line 80–91) | On `CloseRequested`, saves session to disk, then **allows close** (process exits) |
| Frontend hook | `src/components/systray/window.tsx` | `useCloseToTray()` — **never called**; calls `preventDefault()` + `window.hide()` |
| Tray icon | None | No `TrayIconBuilder` or menu exists in Rust |
| ACL permissions | `src-tauri/capabilities/default.json` | `core:tray:default` and `core:menu:default` already granted |
| Icons | `src-tauri/icons/32x32.png`, `128x128.png` | Available for tray use |

## Design Decisions

1. **Handle close-to-tray in Rust** (not the frontend). The Rust `on_window_event` hook already intercepts `CloseRequested` and saves the session. We extend it to also call `window.hide()` and suppress the close via the `WindowEvent::CloseRequested` API. This is more reliable than a frontend JS listener and works even if the WebView is not ready.

2. **Create a system tray icon** in Rust with a `ContextMenu` containing two items:
   - **Show** — reveal and focus the main window
   - **Quit** — save the session, then `app.exit(0)`

3. **Remove or repurpose the unused frontend hook** (`useCloseToTray`) to avoid confusion. It will be deleted since all logic moves to Rust.

4. **Do not add `hideOnClose` in `tauri.conf.json`** because we need conditional behavior: normal close hides, "Quit" from tray actually exits.

## Files to Modify

| File | Action |
|------|--------|
| `src-tauri/src/lib.rs` | Add tray icon + context menu in `.setup()`. Modify `.on_window_event()` to hide window on close (with session save) |
| `src-tauri/src/main.rs` | No change needed |
| `src/components/systray/window.tsx` | Delete (unused, logic moved to Rust) |
| `src/App.tsx` | No change (the hook was never imported) |
| `src-tauri/capabilities/default.json` | No change (permissions already present) |
| `src-tauri/Cargo.toml` | No change (`tray-icon` is already a transitive dependency via `tauri`) |

## Step-by-Step Implementation

### Step 1: Add system tray icon and context menu in `lib.rs`

In the `.setup(|app| { ... })` block (after `app.manage(AppState { ... });`), add the following:

```rust
// ── System Tray ─────────────────────────────────────────────────────────────
use tauri::{tray::{TrayIconBuilder, TrayIconEvent}, menu::{Menu, MenuItem}, Image, Runtime};

// Build the context menu
let show_item = MenuItem::with_id(app, "show", "Show", true, None::<&str>)?;
let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
let menu = Menu::with_items(app, &[&show_item, &quit_item])?;

// Load the 32x32 icon for the tray (Tauri expects a tauri::Image)
let tray_icon = Image::from_bytes(include_bytes!("../icons/32x32.png"))
    .expect("Failed to load tray icon bytes");

// Create and register the tray icon
let _tray = TrayIconBuilder::with_id("main-tray")
    .icon(tray_icon)
    .menu(&menu)
    .show_menu_on_left_click(true)
    .on_menu_event(|tray, event| match event.id().as_ref() {
        "show" => {
            if let Some(window) = tray.app_handle().get_webview_window("main") {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }
        "quit" => {
            // Save session then quit
            let app_handle = tray.app_handle().clone();
            tauri::async_runtime::spawn(async move {
                if let Ok(session_path) = get_session_file_path(&app_handle) {
                    let state = app_handle.state::<AppState>();
                    let _ = state.download_manager.save_session(&session_path).await;
                }
                app_handle.exit(0);
            });
        }
        _ => {}
    })
    .build(app)?;
```

**Key APIs used** (all available in Tauri v2):
- `tauri::tray::TrayIconBuilder`
- `tauri::menu::{Menu, MenuItem}`
- `tauri::Image::from_bytes`
- `tray.app_handle().get_webview_window("main")`

**Edge case**: `get_webview_window("main")` returns `Option<WebviewWindow>` — guard with `if let Some`. On macOS the app id is still "main".

### Step 2: Modify `on_window_event` to hide instead of close

Replace the current `.on_window_event(...)` block with:

```rust
.on_window_event(|window, event| {
    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
        // Save session before hiding
        let app_handle = window.app_handle().clone();
        tauri::async_runtime::spawn(async move {
            if let Ok(session_path) = get_session_file_path(&app_handle) {
                let state = app_handle.state::<AppState>();
                let _ = state.download_manager.save_session(&session_path).await;
            }
        });

        // Prevent the window from closing; hide it to the tray instead
        api.prevent_close();
        let _ = window.hide();
    }
})
```

**Key changes from current code:**
1. Pattern-match `CloseRequested { api, .. }` to destructure the `api` field (gives access to `prevent_close()`).
2. Call `api.prevent_close()` to stop the window from closing.
3. Call `window.hide()` to hide the window (equivalent to minimizing to tray on all platforms).
4. Use `tauri::async_runtime::spawn` (fire-and-forget) instead of `block_on` to avoid deadlocking the event loop while the session save is in flight. The save is best-effort; the app stays alive so if it fails, the next close attempt will retry.

### Step 3: Remove the unused frontend hook

Delete the file `src/components/systray/window.tsx`. It is not imported anywhere (confirmed via grep) and its logic is now fully handled in Rust.

Optionally, if the `src/components/systray/` directory becomes empty, remove the directory as well.

### Step 4: Verify no build breakage

- `cargo check` in `src-tauri/` — confirm `TrayIconBuilder`, `MenuItem`, `Menu`, `Image` are in scope.
- `bun run build` (or `tsc && vite build`) — confirm no dangling imports to the deleted `window.tsx`.
- Run the dev app with `bun run tauri dev`:
  1. Click X → window disappears, tray icon visible.
  2. Right-click tray icon → "Show" → window reappears.
  3. Right-click tray icon → "Quit" → app exits cleanly (session saved).
  4. Close window while a download is in progress → download continues in background.

## Types and Signatures Involved

```rust
// From tauri v2 (already in Cargo.lock via transitive deps)
tauri::tray::TrayIconBuilder<R: Runtime>
tauri::tray::TrayIconEvent
tauri::menu::Menu<R: Runtime>
tauri::menu::MenuItem<R: Runtime>
tauri::Image
tauri::WindowEvent::CloseRequested { api: &mut tauri::window::CloseRequestApi, .. }
tauri::Window::hide(&self) -> tauri::Result<()>
tauri::Window::show(&self) -> tauri::Result<()>
tauri::Window::unminimize(&self) -> tauri::Result<()>
tauri::Window::set_focus(&self) -> tauri::Result<()>
tauri::AppHandle::exit(&self, code: i32)
```

## Edge Cases to Handle

| Case | Handling |
|------|----------|
| User closes window while downloads are active | Window hides; downloads continue in the background daemon. Tray icon keeps the process alive. |
| User clicks "Quit" while downloads are active | Session is saved, then `app.exit(0)` terminates. Active downloads are lost (documented limitation). |
| Tray icon click on macOS vs Windows | `.show_menu_on_left_click(true)` makes left-click show the menu on both platforms. On macOS the menu is the standard NSMenu; on Windows it is the tray context menu. |
| Window is minimized (not hidden) before close | `window.hide()` works regardless of current minimization state. |
| Session save fails (disk full, permissions) | `let _ = ...` swallows the error. The app does not crash. Next close attempt will retry. |
| Multiple `CloseRequested` events in rapid succession | `api.prevent_close()` + `window.hide()` are idempotent; the window is already hidden after the first call. |
| `get_webview_window("main")` returns `None` | Guarded with `if let Some`. No panic. |

## Verification Checklist

- [ ] `cargo check --manifest-path src-tauri/Cargo.toml` passes
- [ ] `bun run build` (TypeScript + Vite) passes
- [ ] Dev run: close window → tray icon appears, process stays alive
- [ ] Tray right-click → "Show" → window restored and focused
- [ ] Tray right-click → "Quit" → app exits, session file written
- [ ] Active downloads continue while window is hidden
- [ ] No TypeScript errors from deleted `window.tsx` import
