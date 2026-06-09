use {
    anyhow::Error,
    axum::{
        Router,
        extract::{
            State, WebSocketUpgrade,
            ws::{CloseFrame, Message, WebSocket},
        },
        response::IntoResponse,
        routing::get,
    },
    demo::{App, config::AppConfig},
    ferrum::{
        Deg, Instance, Quaternion, Rotation3, TypeModel, Vector3, math::TransformDelta,
        models::ModelDesc,
    },
    shared::structs::RpiDemo,
    std::{
        cell::RefCell, collections::HashMap, rc::Rc, result::Result::Ok, sync::mpsc, time::Duration,
    },
    tokio::{net::TcpListener, time::Interval},
    tsl2591_rs::driver::SensorReading,
};

#[derive(Clone)]
struct DemoState {
    data_sender: mpsc::Sender<RpiDemo>,
}

fn main() -> anyhow::Result<(), Error> {
    let demo_models: Rc<RefCell<HashMap<&str, usize>>> = Rc::new(RefCell::new(HashMap::new()));
    let demo_models_update: Rc<RefCell<HashMap<&str, usize>>> = demo_models.clone();

    let (tx, rx) = std::sync::mpsc::channel::<RpiDemo>();

    std::thread::spawn(move || {
        if let Ok(rt) = tokio::runtime::Runtime::new() {
            rt.block_on(up_websokets(tx)).unwrap();
        }
    });

    let app_config: AppConfig = AppConfig::new(Some(ferrum::PhysicalSize::new(500, 500)));

    // Estado que persiste entre frames (UpdateFn es FnMut)
    let mut light_x: f32 = 0.0; // posición acumulada en X
    let mut dir: f32 = 1.0; // sentido del movimiento (+1 / -1)
    let mut last_lux: f32 = 0.0; // último lux recibido del sensor

    App::new(app_config)
        .ferrum_setup(move |state| setup(state, &demo_models))
        .ferrum_update(move |state| {
            update(
                state,
                &demo_models_update,
                &rx,
                &mut light_x,
                &mut dir,
                &mut last_lux,
            )
        })
        .run()
}

fn update(
    state: &mut ferrum::State,
    demo_models: &Rc<RefCell<HashMap<&str, usize>>>,
    rx: &mpsc::Receiver<RpiDemo>,
    light_x: &mut f32,
    dir: &mut f32,
    last_lux: &mut f32,
) {
    let demo_models = demo_models.borrow_mut();

    let now: web_time::Instant = web_time::Instant::now();
    let dt: web_time::Duration = now - state.last_render_time;
    state.last_render_time = now;

    while let Ok(new_data) = rx.try_recv() {
        let light: SensorReading = new_data.light;
        *last_lux = light.lux;
    }

    const SPEED: f32 = 5.0;
    let dx: f32 = *dir * SPEED * dt.as_secs_f32();
    *light_x += dx;
    if *light_x >= 10.0 {
        *light_x = 10.0;
        *dir = -1.0;
    } else if *light_x <= 0.0 {
        *light_x = 0.0;
        *dir = 1.0;
    }

    let transform_delta: TransformDelta = TransformDelta::new(
        Vector3::new(dx, 0.0, 0.0),
        Quaternion::new(0.0, 0.0, 0.0, 0.0),
        Vector3::new(0.0, 0.0, 0.0),
    );

    if let Some(light_id) = demo_models.get("venus") {
        state
            .light_handle()
            .move_flare_object_light(state, light_id, transform_delta, *last_lux);
    } else {
        log::error!("Invalid ID");
    };
}

fn setup(state: &mut ferrum::State, demo_models: &Rc<RefCell<HashMap<&str, usize>>>) {
    let mut demo_models = demo_models.borrow_mut();

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
    demo_models.insert("plant", ingot.id);

    let floor: ModelDesc = ModelDesc::new(
        "floor/floor.obj",
        vec![Instance::default()],
        TypeModel::StaticObj,
    );

    let ingot: ferrum::Ingot<ferrum::models::Model> = state.spawn_model(floor);
    demo_models.insert("floor", ingot.id);

    let venus: ModelDesc = ModelDesc::new(
        "sun/venus.obj",
        vec![Instance::default()],
        TypeModel::PointOfLight,
    );

    let ingot: ferrum::Ingot<ferrum::models::Model> = state.spawn_model(venus);
    demo_models.insert("venus", ingot.id);
}

async fn up_websokets(tx: mpsc::Sender<RpiDemo>) -> Result<(), anyhow::Error> {
    let app: Router = Router::new()
        .route("/demo", get(websocket_handler))
        .with_state(DemoState { data_sender: tx });

    let listener: TcpListener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    println!("Listening on 0.0.0.0:3000/demo");

    axum::serve(listener, app).await?;

    Ok(())
}

#[axum::debug_handler]
async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(state): State<DemoState>,
) -> impl IntoResponse {
    ws.on_failed_upgrade(|error| println!("Error upgrading websocket: {}", error))
        .on_upgrade(move |socket| async move {
            if let Err(e) = handle_socket(socket, state).await {
                eprintln!("Socket error: {e}");
            }
        })
}

async fn handle_socket(mut socket: WebSocket, state: DemoState) -> anyhow::Result<()> {
    let mut interval: Interval = tokio::time::interval(Duration::from_secs(30));

    loop {
        tokio::select! {
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Binary(bytes))) => {
                         let (data_received, _): (RpiDemo, _) =
                            bincode::serde::decode_from_slice(&bytes, bincode::config::standard())
                                .map_err(|e| anyhow::anyhow!("Deserialize error: {e}"))?;
                        state.data_sender.send(data_received)?;
                    }
                    Some(Ok(Message::Close(reason))) => {
                        println!("Client closed: {:?}", reason);
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
