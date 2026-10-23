use tokio::sync::watch;
use tokio::time::{Duration, timeout};

#[derive(Clone, Copy, Debug, PartialEq)]
enum Command {
    Nothing,
    Pause,
}

#[tokio::test]
async fn receiver_created_after_pause_misses_that_notification() {
    let (tx, rx) = watch::channel(Command::Nothing);

    // Simulate the segment's initial command check.
    assert_eq!(*rx.borrow(), Command::Nothing);

    // Pause arrives between that check and the helper's receiver clone.
    tx.send(Command::Pause).unwrap();

    // Simulate cloning the receiver inside get_next_chunk_or_cancel().
    let mut late_receiver = rx.clone();

    // The clone starts with the latest value already visible to it.
    assert_eq!(*late_receiver.borrow(), Command::Pause);

    // It waits for another change rather than reporting the existing Pause.
    assert!(
        timeout(Duration::from_millis(20), late_receiver.changed())
            .await
            .is_err()
    );
}
