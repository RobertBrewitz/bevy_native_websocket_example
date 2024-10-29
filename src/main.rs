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
    app.add_systems(Startup, (connect_to_endpoint, startup_system));
    app.add_systems(
        Update,
        (log_message_received, demonstrate_websocket_is_non_blocking),
    );

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

#[derive(Component)]
struct LogTimer(Timer);

fn startup_system(mut cmd: Commands) {
    cmd.spawn(LogTimer(Timer::from_seconds(5.0, TimerMode::Repeating)));
}

fn demonstrate_websocket_is_non_blocking(time: Res<Time>, mut timer: Query<&mut LogTimer>) {
    for mut timer in timer.iter_mut() {
        timer.0.tick(time.delta());

        if timer.0.finished() {
            println!("This is a non-blocking websocket implementation");
        }
    }
}
