use std::{fmt::Display, ops::Deref, sync::Arc};

use futures::TryFutureExt;
use irc::proto::{Command, Message};
use tokio::sync::{broadcast, mpsc};
use tracing::{debug, error, info};

use crate::error::BanchoIrcError;

async fn background_task(
    channel: Arc<ChannelType>,
    mut receiver: broadcast::Receiver<Message>,
    sender: mpsc::Sender<Command>,
    ch_receiver_tx: broadcast::Sender<(String, String)>,
    mut ch_sender_rx: mpsc::Receiver<Command>,
) {
    let target = channel.deref().deref();
    info!("Listening {channel}");

    loop {
        let res = async {
            tokio::select! {
                msg = receiver.recv() => {
                    channel.handle_msg(msg?, &ch_receiver_tx)?;
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
            Err(e) => error!("Error caught on handling channel {target}: {e:?}"),
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
    channel: Arc<ChannelType>,
    sender: mpsc::Sender<Command>,
    receiver: broadcast::Receiver<Message>,
) -> Result<(broadcast::Receiver<(String, String)>, mpsc::Sender<Command>), BanchoIrcError> {
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

#[derive(Debug)]
pub enum ChannelType {
    Public(String),
    Private(String),
}

impl Display for ChannelType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                ChannelType::Public(s) => format!("[{s}]"),
                ChannelType::Private(s) => format!("[PM] {s}"),
            }
        )
    }
}

impl Deref for ChannelType {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        match self {
            ChannelType::Public(s) | ChannelType::Private(s) => s.as_str(),
        }
    }
}

impl ChannelType {
    #[inline]
    fn handle_msg(
        &self,
        msg: Message,
        receiver_tx: &broadcast::Sender<(String, String)>,
    ) -> Result<(), anyhow::Error> {
        if !matches!(&msg.command, Command::PRIVMSG(_, _)) {
            return Ok(());
        }

        let nickname = msg.source_nickname().map(str::to_string);
        match self {
            ChannelType::Public(s) => match (msg.command, nickname) {
                (Command::PRIVMSG(ch, context), Some(nickname)) if ch.as_str() == s => {
                    debug!("[{s}] {nickname}: {context}");
                    receiver_tx.send((nickname, context))?;
                    Ok(())
                }
                _ => Ok(()),
            },
            ChannelType::Private(s) => match msg.command {
                Command::PRIVMSG(_, context) if msg.source_nickname() == Some(&s) => {
                    debug!("[PM] {s}: {context}");
                    receiver_tx.send((s.to_string(), context))?;
                    Ok(())
                }
                _ => Ok(()),
            },
        }
    }
}

#[derive(Debug)]
pub struct Channel {
    receiver: broadcast::Receiver<(String, String)>,
    sender: mpsc::Sender<Command>,
    ch_type: Arc<ChannelType>,
}

impl Channel {
    pub async fn new(
        channel: ChannelType,
        sender: mpsc::Sender<Command>,
        receiver: broadcast::Receiver<Message>,
    ) -> Result<Arc<Channel>, BanchoIrcError> {
        let channel = Arc::new(channel);
        let (receiver, sender) =
            handle_channel_msg_stream(channel.clone(), sender, receiver).await?;
        Ok(Arc::new(Channel {
            ch_type: channel,
            receiver,
            sender,
        }))
    }
    pub fn name(&self) -> &str {
        &self.ch_type
    }
    pub async fn send_message(&self, context: String) -> Result<(), BanchoIrcError> {
        self.sender
            .send(Command::PRIVMSG(
                self.ch_type.deref().deref().to_string(),
                context,
            ))
            .map_err(Into::into)
            .await
    }

    pub fn receiver(&self) -> broadcast::Receiver<(String, String)> {
        self.receiver.resubscribe()
    }
}
