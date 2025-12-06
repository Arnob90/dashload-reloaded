use crate::statuses::DownloadStatus;
use crate::{idownloaditem::IRunningDownloadItem, terminal_download_item::TerminalDownloadItem};
use std::sync::Arc;
use tokio::sync::RwLock;
pub enum DownloadHandle {
    Running(Arc<dyn IRunningDownloadItem>),
    Terminal(TerminalDownloadItem),
}
pub struct DownloadItem {
    pub handle: RwLock<DownloadHandle>,
}

impl DownloadItem {
    pub async fn new(handle: DownloadHandle) -> Arc<Self> {
        let to_return = Arc::new(Self {
            handle: RwLock::new(handle),
        });
        let to_return_clone = to_return.clone();
        tokio::spawn(to_return_clone.main_loop());
        to_return
    }
    async fn main_loop(self: Arc<Self>) {
        loop {
            // 1) read handle
            let mut receiver = {
                let handle_guard = self.handle.read().await;

                match &*handle_guard {
                    DownloadHandle::Running(running) => running.get_status_reciever(),
                    DownloadHandle::Terminal(_) => return, // already terminal
                }
            };

            // 2) main status loop for "Running" state
            let status = receiver.borrow_and_update().clone();

            // 3) If terminal → swap state
            // TODO: Add stop when done
            if matches!(
                status,
                DownloadStatus::Completed { total: _ }
                    | DownloadStatus::Paused
                    | DownloadStatus::Failed(_)
            ) {
                let mut handle = self.handle.write().await;

                // Only replace if still running (no race)
                if let DownloadHandle::Running(running) = &*handle {
                    let data = running.get_download_data();
                    *handle = DownloadHandle::Terminal(TerminalDownloadItem { data, status });
                }

                return; // done, terminal state
            }

            // 4) Wait for next update (using the SAME receiver)
            if receiver.changed().await.is_err() {
                return; // sender dropped
            }
        }
    }
}
