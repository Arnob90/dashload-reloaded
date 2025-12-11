use tokio::sync::watch;
#[derive(Clone, PartialEq)]
pub enum Command {
    Pause,
    Stop,
}
pub type CommandMaybe = Option<Command>;

pub fn construct_command_sender_reciever()
-> (watch::Sender<CommandMaybe>, watch::Receiver<CommandMaybe>) {
    let initial_state: Option<Command> = None;
    watch::channel(initial_state)
}
