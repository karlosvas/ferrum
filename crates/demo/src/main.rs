use {
    anyhow::Error,
    axum::{
        Router,
        body::Bytes,
        extract::{
            State, WebSocketUpgrade,
            ws::{CloseFrame, Message, WebSocket},
        },
        response::IntoResponse,
        routing::get,
    },
    shared::structs::RpiDemo,
    std::{result::Result::Ok, sync::mpsc, time::Duration},
    tokio::{net::TcpListener, sync::broadcast, time::Interval},
};

#[derive(Clone)]
struct DemoState {
    data_sender: mpsc::Sender<RpiDemo>,
    /// Rebroadcasts every binary packet from the RPi to all connected
    /// clients (the web viewers of the wasm build).
    broadcast: broadcast::Sender<Bytes>,
}

fn main() -> anyhow::Result<(), Error> {
    // Loads RPI_USER/RPI_HOST/IP_HOST from .env to prefill the SSH panel.
    dotenvy::dotenv().ok();

    let (tx, rx) = std::sync::mpsc::channel::<RpiDemo>();

    // The websocket server lives in its own thread and NEVER takes the demo
    // down: without the RPi the demo still starts and works in manual mode
    // with sliders.
    std::thread::spawn(move || match tokio::runtime::Runtime::new() {
        Ok(rt) => {
            if let Err(e) = rt.block_on(up_websokets(tx)) {
                log::error!("WebSocket server error ({e}); demo keeps running in manual mode");
            }
        }
        Err(e) => log::error!("Could not create the tokio runtime ({e}); manual mode"),
    });

    demo::scene::build_app(rx).run()
}

async fn up_websokets(tx: mpsc::Sender<RpiDemo>) -> Result<(), anyhow::Error> {
    let (broadcast_tx, _) = broadcast::channel::<Bytes>(32);
    let app: Router = Router::new()
        .route("/demo", get(websocket_handler))
        .with_state(DemoState {
            data_sender: tx,
            broadcast: broadcast_tx,
        });

    let listener: TcpListener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    log::info!("Listening on 0.0.0.0:3000/demo");

    axum::serve(listener, app).await?;

    Ok(())
}

#[axum::debug_handler]
async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(state): State<DemoState>,
) -> impl IntoResponse {
    ws.on_failed_upgrade(|error| log::error!("Error upgrading websocket: {}", error))
        .on_upgrade(move |socket| async move {
            if let Err(e) = handle_socket(socket, state).await {
                log::error!("Socket error: {e}");
            }
        })
}

async fn handle_socket(mut socket: WebSocket, state: DemoState) -> anyhow::Result<()> {
    let mut interval: Interval = tokio::time::interval(Duration::from_secs(30));
    let mut viewers: broadcast::Receiver<Bytes> = state.broadcast.subscribe();

    loop {
        tokio::select! {
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Binary(bytes))) => {
                         let (data_received, _): (RpiDemo, _) =
                            bincode::serde::decode_from_slice(&bytes, bincode::config::standard())
                                .map_err(|e| anyhow::anyhow!("Deserialize error: {e}"))?;
                        state.data_sender.send(data_received)?;
                        // Forward the packet as-is to the web viewers.
                        let _ = state.broadcast.send(bytes);
                    }
                    Some(Ok(Message::Close(reason))) => {
                        log::info!("Client closed: {:?}", reason);
                        return Ok(());
                    }
                    Some(Err(e)) => {
                        send_close_message(socket, 1011, &format!("Error: {}", e)).await;
                        return Ok(());
                    }
                    None => return Ok(()),
                    Some(Ok(_)) => {}
                }
            }
            forwarded = viewers.recv() => {
                match forwarded {
                    Ok(bytes) => {
                        if socket.send(Message::Binary(bytes)).await.is_err() {
                            return Ok(());
                        }
                    }
                    // Slow viewer: old packets get dropped, which is fine.
                    Err(broadcast::error::RecvError::Lagged(_)) => {}
                    Err(broadcast::error::RecvError::Closed) => return Ok(()),
                }
            }
            _ = interval.tick() => {
                if socket.send(Message::Ping(vec![].into())).await.is_err() {
                    return Ok(());
                }
            }
        }
    }
}

async fn send_close_message(mut socket: WebSocket, code: u16, reason: &str) {
    _ = socket
        .send(Message::Close(Some(CloseFrame {
            code,
            reason: reason.into(),
        })))
        .await;
}
