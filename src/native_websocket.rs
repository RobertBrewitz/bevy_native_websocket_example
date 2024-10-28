use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{handshake::HandshakePlugin, AppSignal};

pub struct NativeWebsocketPlugin;

impl Plugin for NativeWebsocketPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                send_init_message,
                send_message_system,
                receive_message_system,
            ),
        );

        app.add_plugins(HandshakePlugin);
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum WebsocketMessage {
    // Serializes into { type: "connection_init" }
    ConnectionInit,

    // Serializes into { type: "connection_ack", payload: { connectionTimeoutMs: 10000 } }
    ConnectionAck(AppSyncConnectionAckPayload),

    // Serializes into { type: "ka" }
    #[serde(rename = "ka")]
    KeepAlive,

    // TODO: Implement other AppSync messages according to official docs if appsync is used
    // Not AppSync messages, just some examples
    SendMessage {
        message: String,
    },
    ReceiveMessage {
        message: String,
    },
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AppSyncConnectionAckPayload {
    #[serde(rename = "connectionTimeoutMs")]
    pub connection_timout_ms: u64,
}

#[derive(Component)]
pub struct NativeWebsocket {
    pub socket: tungstenite::WebSocket<native_tls::TlsStream<mio::net::TcpStream>>,
}

fn send_init_message(mut websocket_q: Query<&mut NativeWebsocket, Added<NativeWebsocket>>) {
    for mut socket in websocket_q.iter_mut() {
        let message = serde_json::to_string(&WebsocketMessage::ConnectionInit).unwrap();
        println!("Sending init message: {}", message);
        match socket.socket.send(tungstenite::Message::Text(message)) {
            Ok(_) => {
                println!("Init message sent");
            }
            Err(err) => {
                debug!("Failed to send init message: {}", err);
            }
        }
    }
}

fn send_message_system(
    mut websocket_q: Query<&mut NativeWebsocket>,
    mut app_signal_reader: EventReader<AppSignal>,
) {
    for event in app_signal_reader.read() {
        if let AppSignal::SendMessage(message) = event {
            for mut socket in websocket_q.iter_mut() {
                let data = serde_json::to_string(&message).unwrap();
                println!("Sending message: {}", data);
                match socket.socket.send(tungstenite::Message::Text(data)) {
                    Ok(_) => {
                        println!("Message sent");
                    }
                    Err(err) => {
                        debug!("Failed to send message: {}", err);
                    }
                }
            }
        }
    }
}

fn receive_message_system(
    mut app_signal_writer: EventWriter<AppSignal>,
    mut websocket_q: Query<&mut NativeWebsocket>,
) {
    for mut socket in websocket_q.iter_mut() {
        while let Ok(message) = socket.socket.read() {
            match message {
                tungstenite::Message::Text(message) => {
                    println!("Received text message: {}", message);
                    match serde_json::from_str::<WebsocketMessage>(&message) {
                        Ok(message) => {
                            app_signal_writer.send(AppSignal::ReceiveMessage(message));
                        }
                        Err(err) => {
                            debug!("Failed to parse websocket message: {}", err);
                        }
                    }
                }
                tungstenite::Message::Close(_) => {
                    app_signal_writer.send(AppSignal::WebsocketDisconnected);
                    break;
                }
                kind => {
                    debug!("Received unknown message: {:?}", kind);
                }
            }
        }
    }
}
