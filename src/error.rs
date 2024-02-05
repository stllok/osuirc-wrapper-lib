use irc::proto::Command;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum BanchoIrcError {
    #[error(transparent)]
    SendMsgError(#[from] tokio::sync::mpsc::error::SendError<Command>),
    #[error(transparent)]
    RecvMsgError(#[from] tokio::sync::broadcast::error::RecvError),
    #[error(transparent)]
    IrcError(#[from] irc::error::Error),
    #[error("IRC login fail, details: {0}")]
    IrcLoginFailure(String),
    #[error("Channel does not exist, details: {0}")]
    ChannelDoesNotExists(String),
    #[error(transparent)]
    IrcConfigError(#[from] irc::error::ConfigError),
    #[error(transparent)]
    IrcTomlError(#[from] irc::error::TomlError),
    #[error("There are exception during creating match: {0}")]
    CreateMatchError(&'static str),
    #[error("Player not exists in this match")]
    MatchPlayerNotExists,
    #[error(transparent)]
    RegexError(#[from] regex::Error),
    #[error(transparent)]
    TokioAcquireError(#[from] tokio::sync::AcquireError),
    #[error(transparent)]
    Unclassified(#[from] anyhow::Error),
}
