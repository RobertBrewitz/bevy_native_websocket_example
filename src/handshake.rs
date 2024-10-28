use std::{env, str::FromStr};

use tungstenite::{client::IntoClientRequest, handshake::client::generate_key};
use url::Url;
use base64::Engine;

use bevy::{ecs::world::CommandQueue, prelude::*, tasks::{block_on, poll_once}};

use crate::{native_websocket::NativeWebsocket, AppSignal};

pub struct HandshakePlugin;

impl Plugin for HandshakePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (handshake, handle_handshake_task));
    }
}

#[derive(Component)]
pub struct WebsocketHandshakeTask(pub bevy::tasks::Task<CommandQueue>);

// Adds the websocket to the ECS world by merging the handshake command queue into the world
pub fn handle_handshake_task(
    mut cmd: Commands,
    mut settings_read_tasks: Query<(Entity, &mut WebsocketHandshakeTask)>,
    mut app_signal_writer: EventWriter<AppSignal>,
) {
    for (entity, mut task) in settings_read_tasks.iter_mut() {
        if let Some(mut command_queue) = block_on(poll_once(&mut task.0)) {
            cmd.append(&mut command_queue);
            cmd.entity(entity).despawn_recursive();
            app_signal_writer.send(AppSignal::WebsocketConnected);
        }
    }
}

pub fn handshake(
    mut cmd: Commands,
    mut app_signal_event_reader: EventReader<AppSignal>,
) {
    for event in app_signal_event_reader.read() {
        if let AppSignal::ConnectWebsocket(auth_token) = event {
            let token = auth_token.clone();
            let task_pool = bevy::tasks::IoTaskPool::get();
            let task = task_pool.spawn(async move {
                let mut command_queue = CommandQueue::default();

                let req = WebsocketRequest::from_token_for_appsync(token);

                let tls_stream = tls_stream_handshake()
                    .expect("Could not setup non-blocking tls stream");

                let client_req = match req.into_client_request() {
                    Ok(req) => {
                        println!("Client request created");
                        req
                    }
                    Err(e) => {
                        eprintln!("Failed to create client request: {}", e);
                        return command_queue;
                    }
                };

                let mut handshake =
                    match tungstenite::handshake::client::ClientHandshake::start(
                        tls_stream, client_req, None,
                    ) {
                        Ok(handshake) => handshake,
                        Err(e) => {
                            eprintln!("Failed to start handshake: {}", e);
                            return command_queue;
                        }
                    };

                let websocket = loop {
                    match handshake.handshake() {
                        Ok(websocket) => {
                            println!("Websocket handshake complete");
                            break websocket
                        },
                        Err(tungstenite::HandshakeError::Interrupted(mh)) => {
                            handshake = mh;
                        }
                        Err(tungstenite::HandshakeError::Failure(err)) => {
                            eprintln!("Failed to handshake: {}", err);
                            return command_queue;
                        }
                    }
                };

                command_queue.push(move |world: &mut World| {
                    world.spawn((
                        Name::new("websocket"),
                        NativeWebsocket {
                            socket: websocket.0,
                        },
                    ));
                });

                command_queue
            });
            cmd.spawn(WebsocketHandshakeTask(task));
        }
    }
}

/// blocking setup of a non-blocking stream
fn tls_stream_handshake(
) -> Result<native_tls::TlsStream<mio::net::TcpStream>, Box<dyn std::error::Error>> {
    // Address and domain
    let realtime_endpoint = env::var("REALTIME_ENDPOINT")?;
    let url = Url::parse(&realtime_endpoint)?;
    let addrs = url.socket_addrs(|| None)?;
    let domain = url.domain().unwrap_or("localhost");
    let addr_ref = addrs.first().expect("No address found");

    // Mio TCP stream
    let mut tcp_stream = mio::net::TcpStream::connect(addr_ref.to_owned())?;

    // TLS Stream
    let tls_connector = native_tls::TlsConnector::new().expect("Could not create TLS connector");

    // NOTE: This might need to be a different usize per connection
    let token = mio::Token(0);

    let mut poll = mio::Poll::new().expect("Could not create poll");
    let mut events = mio::Events::with_capacity(128);

    poll.registry()
        .register(
            &mut tcp_stream,
            token,
            mio::Interest::READABLE | mio::Interest::WRITABLE,
        )
        .expect("Could not register stream with poll");

    let tls_stream = match tls_connector.connect(domain, tcp_stream) {
        Ok(tls_stream) => tls_stream,
        Err(native_tls::HandshakeError::WouldBlock(mut mid_handshake)) => {
            loop {
                // Continue the handshake
                match mid_handshake.handshake() {
                    Ok(tls_stream) => {
                        println!("TLS Hanshake complete");
                        break tls_stream;
                    }
                    Err(native_tls::HandshakeError::WouldBlock(next_mid_handshake)) => {
                        println!("TLS Handshake would block, block and poll for events");
                        mid_handshake = next_mid_handshake;
                        poll.poll(&mut events, None)?;
                        continue;
                    }
                    Err(e) => return Err(Box::new(e)),
                }
            }
        }
        Err(e) => return Err(Box::new(e)),
    };

    Ok(tls_stream)
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct WebsocketHeaders {
    pub authorization: String,
    #[serde(rename = "host")]
    pub host: String,
    #[serde(rename = "x-amz-date")]
    pub date: String,
}

pub struct WebsocketRequest {
    pub uri: String,
    pub host: String,
    pub token: String,
}

impl WebsocketRequest {
    // NOTE: This is specific for AWS AppSync
    pub fn from_token_for_appsync(token: String) -> Self {
        let graphql_endpoint = env::var("GRAPHQL_ENDPOINT").expect("GRAPHQL_ENDPOINT not set");
        let graphql_uri = tungstenite::http::Uri::from_str(&graphql_endpoint).unwrap();
        let realtime_endpoint = env::var("REALTIME_ENDPOINT").expect("REALTIME_ENDPOINT not set");
        let realtime_uri = tungstenite::http::Uri::from_str(&realtime_endpoint).unwrap();
        let json_header = serde_json::to_string(&WebsocketHeaders {
            // NOTE: Adapt to your own authorization header
            authorization: format!("Bearer {}", token),
            host: graphql_uri.host().expect("No host").into(),
            date: chrono::Utc::now().format("%Y%m%dT%H%M%SZ").to_string(),
        })
        .unwrap();
        let realtime_host = realtime_uri.host().expect("Not host").to_string();

        let header = base64::engine::general_purpose::STANDARD.encode(json_header);

        let payload: String = base64::engine::general_purpose::STANDARD.encode("{}");

        let encoded_uri = format!(
            "{}?header={}&payload={}",
            realtime_endpoint, header, payload,
        );

        Self {
            uri: encoded_uri,
            host: realtime_host,
            token: token.to_string(),
        }
    }
}

impl IntoClientRequest for WebsocketRequest {
    fn into_client_request(self) -> tungstenite::Result<tungstenite::handshake::client::Request> {
        let req = tungstenite::http::Request::builder()
            .uri(self.uri.to_string())
            .method(tungstenite::http::Method::GET)
            .header("Host", self.host.to_string())
            .header("Upgrade", "websocket")
            .header("Connection", "Upgrade")
            .header("Sec-WebSocket-Version", "13")
            .header("Sec-WebSocket-Protocol", "graphql-ws")
            .header("Authorization", format!("Bearer {}", self.token))
            .header(
                "Sec-WebSocket-Extensions",
                "permessage-deflate; client_max_window_bits",
            )
            .header("Sec-WebSocket-Key", generate_key())
            .body(())
            .unwrap();

        let res = tungstenite::handshake::client::Request::from(req);

        Ok(res)
    }
}

