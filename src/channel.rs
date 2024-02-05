use std::sync::Arc;

use futures::TryFutureExt;
use irc::proto::{Command, Message};
use tokio::sync::{broadcast, mpsc};
use tracing::{debug, error, info};

use crate::error::IrcMatchError;

#[derive(Debug)]
pub struct Channel {
    receiver: broadcast::Receiver<(String, String)>,
    sender: mpsc::Sender<Command>,
    name: String,
}

async fn background_task(
    channel: String,
    mut receiver: broadcast::Receiver<Message>,
    sender: mpsc::Sender<Command>,
    ch_receiver_tx: broadcast::Sender<(String, String)>,
    mut ch_sender_rx: mpsc::Receiver<Command>,
) {
    info!("Listening {channel}");
    loop {
        let res = async {
            tokio::select! {
                msg = receiver.recv() => {
                    let msg = msg?;
                    let nickname = msg.source_nickname().map(str::to_string);
                    match (msg.command, nickname) {
                        (Command::PRIVMSG(ch, context), Some(nickname)) if ch.as_str() == &channel => {
                            debug!("[{channel}] {nickname}: {context}");
                            ch_receiver_tx.send((nickname, context))?;
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
            Err(e) => error!("Error caught on handling channel {channel}: {e:?}"),
            _ => continue,
        }
    }

    info!("Leaving {channel}");
    sender
        .send(Command::PART(channel.to_string(), None))
        .await
        .expect("¿");

    debug!("Drop {channel} monitoring");
}

async fn handle_channel_msg_stream(
    channel: String,
    sender: mpsc::Sender<Command>,
    receiver: broadcast::Receiver<Message>,
) -> Result<(broadcast::Receiver<(String, String)>, mpsc::Sender<Command>), IrcMatchError> {
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

impl Channel {
    pub async fn new(
        channel: &str,
        sender: mpsc::Sender<Command>,
        receiver: broadcast::Receiver<Message>,
    ) -> Result<Arc<Channel>, IrcMatchError> {
        let (receiver, sender) =
            handle_channel_msg_stream(channel.to_string(), sender, receiver).await?;
        Ok(Arc::new(Channel {
            name: channel.to_string(),
            receiver,
            sender,
        }))
    }
    pub fn name(&self) -> &str {
        self.name.as_str()
    }
    pub async fn send_message(&self, context: String) -> Result<(), IrcMatchError> {
        self.sender
            .send(Command::PRIVMSG(self.name.to_string(), context))
            .map_err(Into::into)
            .await
    }

    pub fn receiver(&self) -> broadcast::Receiver<(String, String)> {
        self.receiver.resubscribe()
    }
}
