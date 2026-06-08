use {
    anyhow::Error, axum::{
        Router,
        extract::{
            WebSocketUpgrade,
            ws::{CloseFrame, Message, WebSocket},
        },
        response::IntoResponse,
        routing::get,
    }, demo::App, ferrum::{Deg, Instance, Quaternion, Rotation3, TypeModel, Vector3, models::ModelDesc}, shared::structs::RpiDemo, std::{result::Result::Ok, time::Duration}, tokio::{net::TcpListener, runtime::Runtime, time::Interval}, tsl2591_rs::driver::SensorReading
};

#[derive(Clone)]
struct DemoState {
    data_sender: mpsc::Sender<(usize, RpiDemo)>,
}

fn main() -> anyhow::Result<(), Error> {
    let (tx, rx) = std::sync::mpsc::channel::<RpiDemo>();

    std::thread::spawn(move || {
        if let Ok(rt) = tokio::runtime::Runtime::new() {
            rt.block_on(up_websokets(tx)).unwrap();
        }
    });

    App::new()
        .ferrum_setup(setup)
        .ferrum_update(move |self, state| {
            if let Some(venus) = self.demo_models.get("venus") {
                while let Ok(new_data) = rx.try_recv() {
                    let light: LightSensor = RpiDemo.light;

                    let new_transform_light: ferrum::math::TransformDelta = ferrum::math::TransformDelta::new(
                            cgmath::Vector3::new(0.0, 0.0, 0.0),
                            cgmath::Quaternion::new(0.0,0.0,0.0, 0.0),
                            cgmath::Vector3::new(0.0,0.0,0.0)
                    );

                    state.move_flare_object_light(venus.id, new_transform_light);
                }
            }
        }).run()
}

fn setup(&mut self, state: &mut ferrum::State) {
    let plant: ModelDesc = ModelDesc::new(
        "plant/plant.obj",
        vec![Instance::new(
            Vector3::new(0.0, 0.0, 0.0),
            Quaternion::from_angle_y(Deg(0.0)),
            Vector3::new(1.0, 1.0, 1.0),
        )],
        TypeModel::StaticObj,
    );
    
    let ingot: ferrum::Ingot<ferrum::models::Model> = state.spawn_model(plant);
    self.demo_models.insert("plant", ingot);
   
    let floor: ModelDesc = ModelDesc::new(
        "floor/floor.obj",
        vec![Instance::default()],
        TypeModel::StaticObj,
    );

    let ingot: ferrum::Ingot<ferrum::models::Model> = state.spawn_model(floor);
    self.demo_models.insert("floor", ingot);

    let venus: ModelDesc = ModelDesc::new(
        "sun/venus.obj",
        vec![Instance::default()],
        TypeModel::PointOfLight,
    );

    let ingot: ferrum::Ingot<ferrum::models::Model> = state.spawn_model(venus);
    self.demo_models.insert("venus", ingot);
}

async fn up_websokets(tx: mpsc::Sender<(usize, Model)>) -> Result<(), anyhow::Error> {
    let app: Router = Router::new()
                .route("/demo", get(websocket_handler))
                .with_state(DemoState {
                    data_sender: tx
                });

    let listener: TcpListener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    println!("Listening on 0.0.0.0:3000/demo");

    axum::serve(listener, app).await?;

    Ok(())
}

#[axum::debug_handler]
async fn websocket_handler(
            ws: WebSocketUpgrade, tx,
            State(state): State<Demo>
        ) -> impl IntoResponse {
    ws.on_failed_upgrade(|error| println!("Error upgrading websocket: {}", error))
        .on_upgrade(handle_socket(tx))
}

async fn handle_socket(
            mut socket: WebSocket,
            State(state): State<Demo>)
 {
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
                        state.data_sender.send(data_received);
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
