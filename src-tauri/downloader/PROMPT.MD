1. Read InPlaceSegment.rs first. 
2. Refactor start_internal into a state machine, but DO NOT REPLACE IT. Instead, output start_internal_refactored, so I can choose to replace by renaming it in a single go. Half of it has been done for you(via the half finished connect function). The idea is:

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DownloadingState {
    Connect,
    Download { ...extra state you want to pass between goes here  },
    Retry,
    Finish,
}

And:
```rust
impl InPlaceSegment {
    // 1. The orchestrator (The State Machine Driver)
    async fn start_internal(&self) -> Result<(), DownloadRunError> {
        // ... setup checks ...
        let mut state = DownloadingState::Connect { downloaded: self.data.last_downloaded, attempts: 0 };
        
        loop {
            state = match state {
                DownloadingState::Connect { ... } => self.handle_connect(...).await?,
                DownloadingState::Stream { ... } => self.handle_stream(...).await?,
                DownloadingState::Backoff { ... } => self.handle_backoff(...).await?,
                DownloadingState::Finished => break,
            };
        }
        Ok(())
    }

    // 2. Isolated Phase 1
    async fn handle_connect(&self, ...) -> Result<DownloadingState, DownloadRunError> { ... }

    // 3. Isolated Phase 2
    async fn handle_stream(&self, ...) -> Result<DownloadingState, DownloadRunError> { ... }

    // 4. Isolated Phase 3
    async fn handle_backoff(&self, ...) -> Result<DownloadingState, DownloadRunError> { ... }
}
```
