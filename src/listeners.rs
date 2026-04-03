//! # D-Bus Listeners

use futures_lite::stream::StreamExt;
use tokio::sync::mpsc;
use zbus::message::Type;
use zbus::{Connection, MatchRule, MessageStream, Result};

use crate::events::Event;
use crate::logind;
use crate::output;

pub async fn listen_sleep(conn: Connection, tx: mpsc::Sender<Event>) -> Result<()> {
    let rule = MatchRule::builder()
        .msg_type(Type::Signal)
        .sender(logind::SERVICE)?
        .path(logind::PATH)?
        .interface(logind::MANAGER_IFACE)?
        .member("PrepareForSleep")?
        .build();

    output::debug("Listening to PrepareForSleep...");

    let mut stream = MessageStream::for_match_rule(rule, &conn, None).await?;

    while let Some(msg_result) = stream.next().await {
        let msg = msg_result?;
        let going_to_sleep: bool = msg.body().deserialize()?;
        
        if tx.send(Event::Sleep { going_to_sleep }).await.is_err() {
            break;
        }
    }

    Ok(())
}

pub async fn listen_lock(
    conn: Connection,
    session_path: zbus::zvariant::OwnedObjectPath,
    tx: mpsc::Sender<Event>,
) -> Result<()> {
    listen_session_signal(conn, session_path, tx, "Lock", Event::Lock).await
}

pub async fn listen_unlock(
    conn: Connection,
    session_path: zbus::zvariant::OwnedObjectPath,
    tx: mpsc::Sender<Event>,
) -> Result<()> {
    listen_session_signal(conn, session_path, tx, "Unlock", Event::Unlock).await
}

async fn listen_session_signal(
    conn: Connection,
    session_path: zbus::zvariant::OwnedObjectPath,
    tx: mpsc::Sender<Event>,
    member: &'static str,
    event: Event,
) -> Result<()> {
    let rule = MatchRule::builder()
        .msg_type(Type::Signal)
        .sender(logind::SERVICE)?
        .path(session_path.as_str())?
        .interface(logind::SESSION_IFACE)?
        .member(member)?
        .build();

    output::debug(format!("Listening to {} on session {}...", member, session_path.as_str()));

    let mut stream = MessageStream::for_match_rule(rule, &conn, None).await?;

    while let Some(msg_result) = stream.next().await {
        let _msg = msg_result?;
        
        if tx.send(event.clone()).await.is_err() {
            break;
        }
    }

    Ok(())
}

pub async fn get_session_path(conn: &Connection) -> Result<zbus::zvariant::OwnedObjectPath> {
    let pid = std::process::id();

    let session_path: zbus::zvariant::OwnedObjectPath = conn
        .call_method(
            Some(logind::SERVICE),
            logind::PATH,
            Some(logind::MANAGER_IFACE),
            "GetSessionByPID",
            &(pid),
        )
        .await?
        .body()
        .deserialize()?;

    output::debug(format!("Session path: {}", session_path.as_str()));

    Ok(session_path)
}
