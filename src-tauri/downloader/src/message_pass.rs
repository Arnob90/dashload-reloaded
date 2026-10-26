// Single consumer single producer pattern with a single message
use std::sync::Arc;
use tokio::sync::Mutex;

/// Receiver half of a single-message channel.
/// Blocks on `wait_for_message()` until a value is sent, then returns it.
pub struct MessageReciever<Message> {
    _state: Mutex<Option<Message>>,
    message_sent_signal: tokio::sync::Notify,
}

impl<Message> MessageReciever<Message> {
    async fn set_current_state(&self, given_state: Message) {
        let mut required_state = self._state.lock().await;
        *required_state = Some(given_state);
        self.message_sent_signal.notify_one();
    }

    /// Waits until a message is sent, then returns and consumes it.
    pub async fn wait_for_message(&self) -> Option<Message> {
        loop {
            self.message_sent_signal.notified().await;
            let mut required_state = self._state.lock().await;
            if required_state.is_some() {
                return required_state.take();
            }
        }
    }
}

/// Sender half of a single-message channel.
/// Clonable; each send overwrites the pending message.
pub struct MessageSender<Message> {
    reciever: Arc<MessageReciever<Message>>,
}

impl<Message> MessageSender<Message> {
    /// Sends a message, overwriting any previously pending one.
    pub async fn send_message(&self, message: Message) {
        self.reciever.set_current_state(message).await;
    }
}

/// Creates a new `(MessageSender, Arc<MessageReciever>)` pair.
pub async fn create_reciever_sender<MessageT>()
-> (MessageSender<MessageT>, Arc<MessageReciever<MessageT>>) {
    let reciever = Arc::new(MessageReciever {
        _state: Mutex::new(None),
        message_sent_signal: tokio::sync::Notify::new(),
    });
    let sender = MessageSender {
        reciever: reciever.clone(),
    };
    (sender, reciever)
}
