use {
    axum::{
        Router,
        extract::{
            WebSocketUpgrade,
            ws::{CloseFrame, Message, WebSocket},
        },
        response::IntoResponse,
        routing::get,
    },
    demo::App,
    ferrum::{Deg, Instance, Quaternion, Rotation3, TypeModel, Vector3},
    shared::structs::RpiDemo,
    std::{result::Result::Ok, time::Duration},
    tokio::{net::TcpListener, time::Interval},
};

fn main() -> anyhow::Result<()> {
    App::new().ferrum_setup(setup).ferrum_update(update).run()
}

fn update(state: &mut ferrum::State) {}

fn setup(state: &mut ferrum::State) {
    state.spawn_model(
        "plant/plant.obj",
        vec![Instance::new(
            Vector3::new(0.0, 0.0, 0.0),
            Quaternion::from_angle_y(Deg(0.0)),
            Vector3::new(1.0, 1.0, 1.0),
        )],
        TypeModel::StaticObj,
    );

    state.spawn_model(
        "floor/floor.obj",
        vec![Instance::default()],
        TypeModel::StaticObj,
    );

    state.spawn_model(
        "sun/venus.obj",
        vec![Instance::default()],
        TypeModel::PointOfLight,
    );
}

async fn up_websokets() -> Result<(), anyhow::Error> {
    let app: Router = Router::new().route("/demo", get(websocket_handler));

    let listener: TcpListener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    println!("Listening on 0.0.0.0:3000/demo");

    axum::serve(listener, app).await?;

    Ok(())
}

#[axum::debug_handler]
async fn websocket_handler(ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_failed_upgrade(|error| println!("Error upgrading websocket: {}", error))
        .on_upgrade(handle_socket)
}

async fn handle_socket(mut socket: WebSocket) {
    let mut interval: Interval = tokio::time::interval(Duration::from_secs(30));

    loop {
        tokio::select! {
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Binary(bytes))) => {
                        let (data_received, _): (RpiDemo, _) = match bincode::serde::decode_from_slice(&bytes, bincode::config::standard()) {
                            Ok(data) => data,
                            Err(e) => {
                                send_close_message(socket, 1011, &format!("Deserialize error: {}", e)).await;
                                return;
                            }
                        };
                        println!("{:?}", data_received);
                    }
                    Some(Ok(Message::Close(reason))) => {
                        println!("Client closed: {:?}", reason);
                        return;
                    }
                    Some(Err(e)) => {
                        send_close_message(socket, 1011, &format!("Error: {}", e)).await;
                        return;
                    }
                    None => return,
                    Some(Ok(_)) => {}
                }
            }
            _ = interval.tick() => {
                if socket.send(Message::Ping(vec![].into())).await.is_err() {
                    return;
                }
            }
        }
    }
}

async fn send_close_message(mut socket: WebSocket, code: u16, reason: &str) {
    _ = socket
        .send(Message::Close(Some(CloseFrame {
            code: code,
            reason: reason.into(),
        })))
        .await;
}
