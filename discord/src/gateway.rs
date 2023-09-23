use std::{
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use futures_util::{
    future::{pending, Either},
    Future, SinkExt, Stream, StreamExt,
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
use tokio_tungstenite::{
    connect_async, tungstenite::Error, tungstenite::Message, MaybeTlsStream, WebSocketStream,
};

use crate::request::Request;

use super::request::{self, HttpRequest, RequestError};
use super::{interaction::AnyInteraction, request::Bot};

struct GatewayState {
    interval: Interval,
    heartbeat_timeout: Option<Instant>,
    ws_stream: WebSocketStream<MaybeTlsStream<TcpStream>>,
    sender: Sender<GatewayEvent>,
    rx_die: ReceiverStream<()>,

    ready: Option<Ready>,
    sequence: Option<u32>,
    token: String,
}

impl GatewayState {
    async fn heartbeat(&mut self) -> std::result::Result<(), Error> {
        let message = serde_json::to_string(&GatewayMessage {
            op: GatewayOpcode::Heartbeat,
            d: self.sequence,
            s: None,
            t: None,
        })
        .unwrap();

        self.ws_stream.send(Message::text(message)).await?;
        self.heartbeat_timeout = Some(Instant::now() + Duration::from_secs(2));
        Ok(())
    }
    async fn run(&mut self) {
        loop {
            let timeout = match self.heartbeat_timeout {
                Some(deadline) => Either::Left(sleep_until(deadline)),
                None => Either::Right(pending()),
            };
            select! {
                _ = self.rx_die.next() => {
                    // manual close
                    break;
                }
                _ = timeout => {
                    // lost connection
                    break;
                }
                _ = self.interval.tick() => {
                    // heartbeat!
                    if self.heartbeat().await.is_err() {
                        break;
                    }
                }
                item = self.ws_stream.next() => {
                    let Some(Ok(item)) = item else {
                        // end of stream
                        break;
                    };
                    match item {
                        Message::Text(s) => {
                            let message: GatewayMessage<Value> = serde_json::from_str(&s).unwrap();
                            match message.op {
                                GatewayOpcode::Dispatch => {
                                    // event happened
                                    self.sequence = message.s;
                                    let event: std::result::Result<GatewayEvent, _> = serde_json::from_str(&s);
                                    match event {
                                        Ok(GatewayEvent::Ready(ready)) => {
                                            self.ready = Some(ready);
                                        }
                                        Ok(event) => {
                                            if self.sender.send(event).await.is_err() {
                                                // receiver is gone
                                                break;
                                            }
                                        }
                                        _ => (),
                                    }
                                }
                                GatewayOpcode::Heartbeat => {
                                    // heartbeat!
                                    if self.heartbeat().await.is_err() {
                                        break;
                                    }
                                }
                                GatewayOpcode::InvalidSession => {
                                    println!("OOP invalid session");
                                    break;
                                }
                                GatewayOpcode::HeartbeatACK => {
                                    self.heartbeat_timeout = None;
                                }
                                GatewayOpcode::Reconnect => {
                                    // resume stream
                                    let (Some(ready), Some(sequence)) = (&self.ready, self.sequence) else {
                                        // we have no resume information
                                        break;
                                    };

                                    let full_url = format!("{}/?v=10&encoding=json", ready.resume_gateway_url);

                                    self.ws_stream.close(None).await.expect("old websocket stream could not close");
                                    (self.ws_stream, _) = connect_async(full_url).await.expect("could not connect");

                                    let resume = serde_json::to_string(&GatewayMessage {
                                        op: GatewayOpcode::Resume,
                                        d: Resume {
                                            token: &self.token,
                                            session_id: &ready.session_id,
                                            seq: sequence,
                                        },
                                        s: None,
                                        t: None,
                                    })
                                    .unwrap();

                                    if self.ws_stream.send(Message::Text(resume)).await.is_err() {
                                        // could not send resume
                                        break;
                                    }
                                }
                                GatewayOpcode::Hello => {
                                    // set heartbeat interval
                                    let hello: std::result::Result<Hello, _> = serde_json::from_value(message.d);
                                    if let Ok(hello) = hello {
                                        let heartbeat_interval = hello.heartbeat_interval;
                                        let offset = rand::thread_rng().gen_range(0..heartbeat_interval);
                                        let start = Instant::now() + Duration::from_millis(offset);
                                        self.interval = interval_at(start, Duration::from_millis(heartbeat_interval));
                                    }
                                }
                                _ => {}
                            }
                        }
                        Message::Close(_) => {
                            // end of stream
                            break;
                        }
                        _ => {}
                    }
                }
            }
        }
        // TODO: reconnect?
    }
}

pub struct Gateway {
    stream: ReceiverStream<GatewayEvent>,
    task: JoinHandle<()>,
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
struct Identify<'a> {
    token: &'a str,
    intents: u32,
    properties: ConnectionProperties,
}

#[derive(Serialize, Debug)]
struct ConnectionProperties {
    os: &'static str,
    browser: &'static str,
    device: &'static str,
}

#[derive(Deserialize, Debug)]
pub struct Ready {
    resume_gateway_url: String,
    session_id: String,
}

#[derive(Serialize, Debug)]
struct Resume<'a> {
    token: &'a str,
    session_id: &'a str,
    seq: u32,
}

const NAME: &str = env!("CARGO_PKG_NAME");

impl Stream for Gateway {
    type Item = GatewayEvent;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if let Poll::Ready(_) = Pin::new(&mut self.task).poll(cx) {
            return Poll::Ready(None);
        }

        if let Poll::Ready(event) = Pin::new(&mut self.stream.next()).poll(cx) {
            return Poll::Ready(event);
        }

        Poll::Pending
    }
}

impl Gateway {
    pub async fn connect(client: &Bot) -> request::Result<Self> {
        let GatewayResponse { url } = HttpRequest::get("/gateway").request(client).await?;
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
                token: client.token(),
                intents: 0,
                properties: ConnectionProperties {
                    os: "linux",
                    browser: NAME,
                    device: NAME,
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
            token: client.token().into(),
        };

        let task = tokio::spawn(async move { state.run().await });

        Ok(Gateway {
            task,
            tx_die,
            stream: ReceiverStream::new(rx_event),
        })
    }

    pub async fn next(&mut self) -> Option<GatewayEvent> {
        StreamExt::next(self).await
    }

    pub async fn close(self) {
        println!("closing gateway");

        if !self.task.is_finished() {
            let _ = self.tx_die.send(()).await;
            let _ = self.task.await;
        }
    }
}
