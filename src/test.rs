use std::time::Duration;

use anyhow::Result;
use dotenvy::dotenv;
use futures::TryFutureExt;
use tracing::{debug, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::client::OsuIrcClient;

fn init_tracer() -> Result<()> {
    dotenv()?;
    tracing_subscriber::registry()
        .with(tracing_subscriber::filter::LevelFilter::DEBUG)
        .with(tracing_subscriber::fmt::Layer::new())
        .try_init()?;
    Ok(())
}

async fn normal_login() -> Result<OsuIrcClient> {
    OsuIrcClient::new(
        std::env::var("IRC_USERNAME")?,
        std::env::var("IRC_PASSWORD")?,
    )
    .map_err(Into::into)
    .await
}

#[tokio::test]
async fn test_login() -> Result<()> {
    init_tracer()?;
    let cli = normal_login().await?;

    debug!("{cli:?}");

    tokio::time::sleep(Duration::from_secs(5)).await;
    Ok(())
}
#[tokio::test]
async fn test_login_failed() -> Result<()> {
    init_tracer()?;

    let res = OsuIrcClient::new("abcd".into(), "1234".into()).await;

    info!("RESULT: {res:?}");

    assert!(res.is_err());
    Ok(())
}

#[tokio::test]
async fn test_join_channel() -> Result<()> {
    init_tracer()?;
    let mut cli = normal_login().await?;
    let ch = cli.join_channel("#osu".into()).await?;
    let mut rx = ch.receiver();
    for _ in 0..5 {
        let msg = rx.recv().await?;
        info!("{}: {}", msg.0, msg.1);
    }
    info!("SUCCESS");
    Ok(())
}

#[tokio::test]
async fn test_join_channel_failed() -> Result<()> {
    init_tracer()?;
    let mut cli = normal_login().await?;
    let res = cli.join_channel("#chinrse".into()).await;
    info!("RESULT: {res:?}");

    assert!(res.is_err());
    Ok(())
}

#[tokio::test]
async fn test_channel_chat() -> Result<()> {
    init_tracer()?;
    let cli = normal_login().await?;
    let ch = cli.new_private_message("Tillerino".into()).await?;
    let mut rx = ch.receiver();
    ch.send_message("!help".into()).await?;
    info!("{:?}", rx.recv().await?);
    Ok(())
}

// #[tokio::test]
// async fn test_channel_chat_to_someone() -> Result<()> {
//     init_tracer()?;
//     let cli = normal_login().await?;
//     let ch = cli.new_private_message("".into()).await?;
//     ch.send_message("Hello world from programming code".into())
//         .await?;
//     tokio::time::sleep(Duration::from_secs(30)).await;
//     ch.send_message("Quit".into()).await?;
//     info!("LEFT!");
//     Ok(())
// }
