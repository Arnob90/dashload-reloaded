use crate::download_data::DownloadData;
use crate::statuses::DownloadStatus;
use crate::{idownloaditem::IRunningDownloadItem, terminal_download_item::TerminalDownloadItem};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
pub enum DownloadHandle {
    Running(Arc<dyn IRunningDownloadItem>),
    Terminal(TerminalDownloadItem),
}
pub struct DownloadItem {
    pub handle: RwLock<DownloadHandle>,
    pub poll_every_ms: u64,
}

impl DownloadItem {
    pub async fn new(handle: DownloadHandle, poll_every_ms: u64) -> Arc<Self> {
        let to_return = Arc::new(Self {
            handle: RwLock::new(handle),
            poll_every_ms,
        });
        let to_return_clone = to_return.clone();
        tokio::spawn(to_return_clone.main_loop());
        to_return
    }
    async fn replace_with_terminal(
        self: Arc<Self>,
        data: DownloadData,
        current_status: DownloadStatus,
    ) {
        let mut lock = self.handle.write().await;
        *lock = DownloadHandle::Terminal(TerminalDownloadItem {
            status: current_status,
            data,
        });
    }
    async fn main_loop(self: Arc<Self>) {
        loop {
            let (current_status, data, is_terminal) = {
                let lock = self.handle.read().await;
                match *lock {
                    DownloadHandle::Running(ref item) => {
                        let status_maybe = item.get_status();
                        if let Ok(status) = status_maybe {
                            (status, item.get_download_data(), false)
                        } else {
                            continue;
                        }
                    }
                    DownloadHandle::Terminal(ref item) => {
                        (item.status.clone(), item.data.clone(), true)
                    }
                }
            };
            if is_terminal {
                return;
            }
            if matches!(
                current_status,
                DownloadStatus::Stopped
                    | DownloadStatus::Paused { .. }
                    | DownloadStatus::Failed(_)
                    | DownloadStatus::Completed { .. }
            ) {
                self.clone()
                    .replace_with_terminal(data, current_status)
                    .await;
            }
            tokio::time::sleep(Duration::from_millis(self.poll_every_ms)).await;
        }
    }
}
