use std::sync::Arc;

use futures::TryFutureExt;
use irc::proto::{Command, Message};
use tokio::sync::{broadcast, mpsc};
use tracing::{debug, error, info};

use crate::error::IrcMatchError;

async fn background_task(
    name: String,
    mut receiver: broadcast::Receiver<Message>,
    sender: mpsc::Sender<Command>,
    ch_receiver_tx: broadcast::Sender<String>,
    mut ch_sender_rx: mpsc::Receiver<Command>,
) {
    info!("Listening PM {name}");
    loop {
        let res = async {
            tokio::select! {
                msg = receiver.recv() => {
                    let msg = msg?;
                    match msg.command {
                        Command::PRIVMSG(_, context) if msg.source_nickname() == Some(&name) => {
                            debug!("[PM] {name}: {context}");
                            ch_receiver_tx.send(context)?;
                        }
                        _ => (),
                    }
                    Ok::<bool, anyhow::Error>(true)
                },
                Some(com) = ch_sender_rx.recv() => {
                    sender.send(com).await?;
                    Ok::<bool, anyhow::Error>(true)
                },
                else => Ok::<bool, anyhow::Error>(false),
            }
        }
        .await;

        match res {
            Ok(false) => break,
            Err(e) => error!("Error caught on handling private message {name}: {e:?}"),
            _ => continue,
        }
    }

    info!("Leaving {name}");
    sender
        .send(Command::PART(name.to_string(), None))
        .await
        .expect("¿");

    debug!("Drop {name} monitoring");
}

async fn handle_channel_msg_stream(
    channel: String,
    sender: mpsc::Sender<Command>,
    receiver: broadcast::Receiver<Message>,
) -> Result<(broadcast::Receiver<String>, mpsc::Sender<Command>), IrcMatchError> {
    let (ch_receiver_tx, ch_receiver_rx) = broadcast::channel(128);
    let (ch_sender_tx, ch_sender_rx) = mpsc::channel(8);

    tokio::spawn(background_task(
        channel,
        receiver,
        sender,
        ch_receiver_tx,
        ch_sender_rx,
    ));

    Ok((ch_receiver_rx, ch_sender_tx))
}

pub struct PrivateMessage {
    name: String,
    receiver: broadcast::Receiver<String>,
    sender: mpsc::Sender<Command>,
}

impl PrivateMessage {
    pub async fn new(
        channel: &str,
        sender: mpsc::Sender<Command>,
        receiver: broadcast::Receiver<Message>,
    ) -> Result<Arc<PrivateMessage>, IrcMatchError> {
        let (receiver, sender) =
            handle_channel_msg_stream(channel.to_string(), sender, receiver).await?;
        Ok(Arc::new(PrivateMessage {
            name: channel.to_string(),
            receiver,
            sender,
        }))
    }
    pub fn name(&self) -> &str {
        self.name.as_str()
    }
    pub async fn send_message(&self, context: String) -> Result<(), IrcMatchError> {
        debug!("PM -> {}: {context}", self.name);
        self.sender
            .send(Command::PRIVMSG(self.name.to_string(), context))
            .map_err(Into::into)
            .await
    }

    pub fn receiver(&self) -> broadcast::Receiver<String> {
        self.receiver.resubscribe()
    }
}
