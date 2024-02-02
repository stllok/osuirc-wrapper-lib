use irc::proto::Command;
use tokio::sync::broadcast;

pub struct Channel {
    name: String,
    receiver: broadcast::Receiver<Command>,
}

impl Channel {
    pub fn name(&self) -> &str {
        self.name.as_ref()
    }
}
