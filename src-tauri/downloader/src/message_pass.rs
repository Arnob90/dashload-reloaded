//Single consumer single producer pattern with a single message
use std::sync::Arc;
use tokio::sync::Mutex;
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

pub struct MessageSender<Message> {
    reciever: Arc<MessageReciever<Message>>,
}

impl<Message> MessageSender<Message> {
    pub async fn send_message(&self, message: Message) {
        self.reciever.set_current_state(message).await;
    }
}
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
