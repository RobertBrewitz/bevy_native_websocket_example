use bevy::prelude::*;

mod handshake;
mod native_websocket;

use native_websocket::{NativeWebsocketPlugin, WebsocketMessage};

fn main() {
    dotenv::dotenv().ok();

    let mut app = App::new();

    app.add_plugins(DefaultPlugins);

    // Native websocket stuff
    app.add_event::<AppSignal>();
    app.add_plugins(NativeWebsocketPlugin);
    app.add_systems(Startup, connect_to_endpoint);
    app.add_systems(Update, log_message_received);

    app.run();
}

fn connect_to_endpoint(mut app_signal_writer: EventWriter<AppSignal>) {
    // TODO: Get an auth token, in my case I use a custom auth lamba for appsync
    //
    // Dunno how you auth with your websocket server
    app_signal_writer.send(AppSignal::ConnectWebsocket("<auth token here>".to_string()));
}

fn log_message_received(mut app_signal_reader: EventReader<AppSignal>) {
    for event in app_signal_reader.read() {
        if let AppSignal::ReceiveMessage(message) = event {
            println!("Received message: {:#?}", message);
        }
    }
}

#[derive(Event)]
pub enum AppSignal {
    ConnectWebsocket(/* auth token */ String),
    WebsocketConnected,
    WebsocketDisconnected,

    SendMessage(WebsocketMessage),
    ReceiveMessage(WebsocketMessage),
}
