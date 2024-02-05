use std::sync::Arc;

use futures::{StreamExt, TryFutureExt, TryStreamExt};
use irc::{
    client::{data::Config, Client, ClientStream, Sender},
    proto::{Command, Message, Response},
};
use tokio::sync::{broadcast, mpsc};
use tracing::{debug, error, info};

use crate::{channel::Channel, error::IrcMatchError, private_message::PrivateMessage};

#[derive(Debug)]
pub struct OsuIrcClient {
    channel: Vec<Arc<Channel>>,
    sender: mpsc::Sender<Command>,
    receiver: broadcast::Receiver<Message>,
}

async fn background_task(
    sender: Sender,
    mut stream: ClientStream,
    mut sender_rx: mpsc::Receiver<Command>,
    receiver_tx: broadcast::Sender<Message>,
) {
    loop {
        let res = async {
            tokio::select! {
                Some(com) = sender_rx.recv() => {
                    sender.send(com)?;
                    Ok::<bool, anyhow::Error>(true)
                },
                Some(com) = stream.next() => {
                    let com = com?;
                    receiver_tx.send(com)?;
                    Ok::<bool, anyhow::Error>(true)
                },
                else => {
                    Ok::<bool, anyhow::Error>(false)
                }
            }
        }
        .await;

        match res {
            Ok(false) => break,
            Err(e) => error!("Error caught on irc message processing: {e:?}"),
            _ => continue,
        }
    }
    debug!("Dropping osu! Bancho IRC monitoring");
}

async fn handle_irc_msg_stream(
    mut cli: Client,
) -> Result<(mpsc::Sender<Command>, broadcast::Receiver<Message>), IrcMatchError> {
    let (receiver_tx, receiver_rx) = broadcast::channel::<Message>(128);
    let (sender_tx, sender_rx) = mpsc::channel::<Command>(128);
    let mut stream = cli.stream()?;
    let sender = cli.sender();

    // login check
    while let Some(com) = stream.try_next().await? {
        match com.command {
            Command::Response(Response::ERR_PASSWDMISMATCH, f) => {
                return Err(IrcMatchError::IrcLoginFailure(f.join(" ")))
            }
            Command::Response(e, f) if e.is_error() => {
                panic!("{}", f.join(" "))
            }
            Command::Response(Response::RPL_WELCOME, _) => break,
            _ => continue,
        }
    }

    tokio::spawn(background_task(sender, stream, sender_rx, receiver_tx));

    Ok((sender_tx, receiver_rx))
}

impl OsuIrcClient {
    pub async fn new(username: String, password: String) -> Result<OsuIrcClient, IrcMatchError> {
        let cli = Client::from_config(Config {
            nickname: Some(username.clone()),
            username: Some(username),
            password: Some(password.clone()),
            nick_password: Some(password),
            server: Some("irc.ppy.sh".into()),
            port: Some(6667),
            use_tls: Some(false),
            burst_window_length: Some(10),
            max_messages_in_burst: Some(5),
            ping_time: Some(120),
            ping_timeout: Some(30),
            ..Default::default()
        })
        .await?;
        cli.identify()?;
        let (sender_tx, receiver_rx) = handle_irc_msg_stream(cli).await?;
        info!("Login to osu! irc success!");
        Ok(OsuIrcClient {
            channel: vec![],
            sender: sender_tx,
            receiver: receiver_rx,
        })
    }
    pub async fn join_channel(&mut self, channel: String) -> Result<Arc<Channel>, IrcMatchError> {
        if let Some(ch) = self.channel.iter().find(|c| c.name() == &channel) {
            return Ok(ch.clone());
        }
        self.sender
            .send(Command::JOIN(channel.clone(), None, Some("cho".into())))
            .map_err(IrcMatchError::SendMsgError)
            .await?;

        loop {
            match self.receiver.recv().await?.command {
                Command::JOIN(ch, _, _) if &ch == &channel => break,
                Command::Response(Response::ERR_NOSUCHCHANNEL, f) => {
                    return Err(IrcMatchError::ChannelDoesNotExists(f.join(" ")))
                }
                Command::Response(e, f) if e.is_error() => {
                    panic!("{}", f.join(" "))
                }
                _ => continue,
            }
        }

        let channel =
            Channel::new(&channel, self.sender.clone(), self.receiver.resubscribe()).await?;

        self.channel.push(channel.clone());

        Ok(channel)
    }

    pub async fn new_private_message(
        &self,
        name: String,
    ) -> Result<Arc<PrivateMessage>, IrcMatchError> {
        PrivateMessage::new(&name, self.sender.clone(), self.receiver.resubscribe()).await
    }
}
