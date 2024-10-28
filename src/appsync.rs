use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AppSyncConnectionAckPayload {
    #[serde(rename = "connectionTimeoutMs")]
    pub connection_timout_ms: u64,
}

// https://serde.rs/enum-representations.html#internally-tagged
// https://docs.aws.amazon.com/appsync/latest/devguide/real-time-websocket-client.html
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum AppSyncRealtimeMessage {
    // Connection
    ConnectionInit,
    ConnectionAck {
        payload: AppSyncConnectionAckPayload,
    },
}
