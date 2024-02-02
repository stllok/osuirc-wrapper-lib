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
        .with(tracing_subscriber::filter::LevelFilter::TRACE)
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

    tokio::time::sleep(Duration::from_secs(30)).await;
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
    cli.join_channel("#chinese".into()).await?;
    info!("EXECUTE join_channel success!");
    tokio::time::sleep(Duration::from_secs(15)).await;

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
