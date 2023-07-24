use std::{pin::Pin, task::Poll, time::Duration};

use futures_util::{
    future::{pending, Either},
    SinkExt, Stream, StreamExt,
};
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use serde_repr::{Deserialize_repr, Serialize_repr};
use tokio::{
    net::TcpStream,
    select,
    sync::mpsc::{self, Sender},
    task::JoinHandle,
    time::{interval_at, sleep_until, Instant, Interval},
};
use tokio_stream::wrappers::ReceiverStream;
use tokio_tungstenite::{connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream};

use crate::request::Client;

use super::request::{self, Request, RequestError};
use super::{interaction::AnyInteraction, request::Discord};

struct GatewayState {
    interval: Interval,
    heartbeat_timeout: Option<Instant>,
    ws_stream: WebSocketStream<MaybeTlsStream<TcpStream>>,
    sender: Sender<GatewayEvent>,
    rx_die: ReceiverStream<()>,

    ready: Option<Ready>,
    sequence: Option<u32>,
}

type Result = std::result::Result<(), ()>;

impl GatewayState {
    async fn heartbeat(&mut self) -> Result {
        let message = serde_json::to_string(&GatewayMessage {
            op: GatewayOpcode::Heartbeat,
            d: self.sequence,
            s: None,
            t: None,
        })
        .unwrap();

        if self.ws_stream.send(Message::text(message)).await.is_err() {
            Err(())
        } else {
            self.heartbeat_timeout = Some(Instant::now() + Duration::from_secs(2));
            Ok(())
        }
    }
    async fn iteration(&mut self) -> Result {
        let timeout = match self.heartbeat_timeout {
            Some(deadline) => Either::Left(sleep_until(deadline)),
            None => Either::Right(pending()),
        };
        select! {
            _ = self.rx_die.next() => {
                // manual close
                return Err(());
            }
            _ = timeout => {
                // lost connection
                return Err(());
            }
            _ = self.interval.tick() => {
                // heartbeat!
                self.heartbeat().await?;
            }
            item = self.ws_stream.next() => {
                let Some(Ok(item)) = item else {
                    // end of stream
                    return Err(());
                };
                match item {
                    Message::Text(s) => {
                        let message: GatewayMessage<Value> = serde_json::from_str(&s).unwrap();
                        match message.op {
                            GatewayOpcode::Dispatch => {
                                // event happened
                                self.sequence = message.s;
                                let event: GatewayEvent = serde_json::from_str(&s).unwrap();
                                match event {
                                    GatewayEvent::Ready(ready) => {
                                        self.ready = Some(ready);
                                    }
                                    event => {
                                        if self.sender.send(event).await.is_err() {
                                            return Err(());
                                        }
                                    }
                                }
                            }
                            GatewayOpcode::Heartbeat => {
                                // heartbeat!
                                self.heartbeat().await?;
                            }
                            GatewayOpcode::InvalidSession => {
                                return Err(());
                            }
                            GatewayOpcode::HeartbeatACK => {
                                self.heartbeat_timeout = None;
                            }
                            GatewayOpcode::Reconnect => {
                                // TODO: try to resume
                                return Err(());
                            }
                            _ => {}
                        }
                    }
                    Message::Close(_) => {
                        // end of stream
                        return Err(());
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }
}

pub struct Gateway {
    stream: ReceiverStream<GatewayEvent>,
    task: JoinHandle<GatewayState>,
    tx_die: Sender<()>,
}

#[derive(Deserialize)]
struct GatewayResponse {
    url: String,
}

#[derive(Deserialize_repr, Serialize_repr, Debug, Copy, Clone, PartialEq, Eq)]
#[repr(u8)]
enum GatewayOpcode {
    Dispatch = 0,
    Heartbeat = 1,
    Identify = 2,
    Resume = 6,
    Reconnect = 7,
    InvalidSession = 9,
    Hello = 10,
    HeartbeatACK = 11,
}

#[derive(Deserialize, Serialize, Debug)]
struct GatewayMessage<T> {
    op: GatewayOpcode,
    d: T,
    #[serde(skip_serializing_if = "Option::is_none")]
    s: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    t: Option<String>,
}

#[derive(Deserialize, Debug)]
#[serde(tag = "t", content = "d", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GatewayEvent {
    Ready(Ready),
    InteractionCreate(AnyInteraction),
}

#[derive(Deserialize, Debug)]
struct Hello {
    heartbeat_interval: u64,
}

#[derive(Serialize, Debug)]
struct Identify {
    token: String,
    intents: u32,
    properties: ConnectionProperties,
}

#[derive(Serialize, Debug)]
struct ConnectionProperties {
    os: String,
    browser: String,
    device: String,
}

#[derive(Deserialize, Debug)]
pub struct Ready {
    resume_gateway_url: String,
    session_id: String,
}

#[derive(Serialize, Debug)]
struct Resume {
    token: String,
    session_id: String,
    seq: u32,
}

impl Stream for Gateway {
    type Item = GatewayEvent;

    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        if self.task.is_finished() {
            Poll::Ready(None)
        } else {
            self.stream.poll_next_unpin(cx)
        }
    }
}

const NAME: &str = env!("CARGO_PKG_NAME");

impl Gateway {
    pub async fn connect(client: &Discord) -> request::Result<Self> {
        let GatewayResponse { url } = client.request(Request::get("/gateway".to_owned())).await?;
        let full_url = url + "/?v=10&encoding=json";

        let (mut ws_stream, _) = connect_async(full_url).await.expect("could not connect");
        let hello = ws_stream
            .next()
            .await
            .expect("no message")
            .expect("no connection")
            .into_text()
            .expect("not utf8");

        let GatewayMessage {
            d: Hello { heartbeat_interval },
            op: _,
            s: _,
            t: _,
        } = serde_json::from_str(&hello).expect("unexpected message");

        let identify = serde_json::to_string(&GatewayMessage {
            op: GatewayOpcode::Identify,
            d: Identify {
                token: client.token().to_owned(),
                intents: 0,
                properties: ConnectionProperties {
                    os: "linux".to_owned(),
                    browser: NAME.to_owned(),
                    device: NAME.to_owned(),
                },
            },
            s: None,
            t: None,
        })
        .unwrap();

        if ws_stream.send(Message::Text(identify)).await.is_err() {
            return Err(RequestError::InvalidSession);
        }

        let offset = rand::thread_rng().gen_range(0..heartbeat_interval);
        let start = Instant::now() + Duration::from_millis(offset);
        let interval = interval_at(start, Duration::from_millis(heartbeat_interval));

        let (tx_event, rx_event) = mpsc::channel(16);
        let (tx_die, rx_die) = mpsc::channel(1);

        let mut state = GatewayState {
            interval,
            sequence: None,
            heartbeat_timeout: None,
            ws_stream,
            rx_die: ReceiverStream::new(rx_die),
            sender: tx_event,
            ready: None,
        };

        let task = tokio::spawn(async move {
            while state.iteration().await.is_ok() {}
            state
        });

        Ok(Gateway {
            task,
            tx_die,
            stream: ReceiverStream::new(rx_event),
        })
    }

    pub async fn close(self) {
        self.tx_die.send(()).await.unwrap();

        let mut state = self.task.await.unwrap();
        state.ws_stream.close(None).await.unwrap();
    }
}
