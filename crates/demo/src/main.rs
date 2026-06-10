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
    cgmath::{InnerSpace, Vector2},
    demo::{App, config::AppConfig},
    ferrum::{Deg, Instance, Quaternion, Rotation3, TypeModel, Vector3, models::ModelDesc},
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
    let mut last_lux: f32 = 0.0; // último lux recibido del sensor
    let mut light_pos: Vector3<f32> = Vector3::new(0.0, 0.0, 0.0); // última posición de la cámara
    let mut wind_target: Vector2<f32> = Vector2::new(0.0, 0.0); // viento objetivo (de los micros)
    let mut wind_current: Vector2<f32> = Vector2::new(0.0, 0.0); // viento suavizado por frame
    let mut wind_last_t: web_time::Instant = web_time::Instant::now();
    let mut silent_ticks: u32 = 0; // paquetes seguidos de silencio (para no cortar el viento entre respiraciones)

    App::new(app_config)
        .ferrum_setup(move |state| setup(state, &demo_models))
        .ferrum_update(move |state| {
            update(
                state,
                &demo_models_update,
                &rx,
                &mut last_lux,
                &mut light_pos,
                &mut wind_target,
                &mut wind_current,
                &mut wind_last_t,
                &mut silent_ticks,
            )
        })
        .run()
}

fn update(
    state: &mut ferrum::State,
    demo_models: &Rc<RefCell<HashMap<&str, usize>>>,
    rx: &mpsc::Receiver<RpiDemo>,
    last_lux: &mut f32,
    light_pos: &mut Vector3<f32>,
    wind_target: &mut Vector2<f32>,
    wind_current: &mut Vector2<f32>,
    wind_last_t: &mut web_time::Instant,
    silent_ticks: &mut u32,
) {
    let demo_models = demo_models.borrow_mut();

    state.last_render_time = web_time::Instant::now();

    // Consumir todos los datos pendientes y quedarnos con el más reciente.
    while let Ok(new_data) = rx.try_recv() {
        let light: SensorReading = new_data.light;
        *last_lux = light.lux;
        *light_pos = Vector3::new(
            new_data.camera.x as f32,
            new_data.camera.y as f32,
            new_data.camera.z as f32,
        );

        // Viento a partir de los 4 micrófonos. La RPi ya envía la actividad por
        // encima del suelo de ruido de cada canal:
        //   canal 1=adelante, 2=derecha, 3=atrás, 4=izquierda  →  índices 0..3.
        //
        // Soplar es acústicamente ruidoso: el micro soplado satura (~32000) pero
        // los vecinos también suben (2000-6000), así que una resta diferencial
        // da direcciones caóticas. En su lugar: WINNER-TAKE-ALL con dominancia.
        // Solo cambia la dirección si un micro supera claramente al segundo; si
        // la lectura es ambigua se mantiene el viento anterior (sin bandazos).
        //
        // El aire viaja DESDE el micro ganador HACIA la planta: soplar el de la
        // derecha empuja las hojas hacia la izquierda. Si en pantalla sale
        // invertido (depende de la orientación de tu escena), invierte signos.
        const NOISE_GATE: f32 = 1500.0; // por debajo de esto, silencio
        const DOMINANCE: f32 = 1.5; // el ganador debe superar al 2º por este factor
        // Amplitud a la que el balanceo es máximo. Un soplido fuerte satura el
        // ADC en ~33000; antes estaba en 8000 y CUALQUIER soplido llegaba al
        // máximo — por eso fuerte y suave se veían igual de intensos.
        const MAX_RAW: f32 = 30000.0;
        // Soplar no es continuo: entre respiraciones hay 1-3 paquetes a cero.
        // En vez de apagar el viento al primer silencio, se mantiene el último
        // objetivo durante unos paquetes para que la ráfaga no parpadee.
        const SILENT_HOLD: u32 = 4;
        let m = &new_data.microphone;
        let vals: [f32; 4] = [
            m[0].raw as f32,
            m[1].raw as f32,
            m[2].raw as f32,
            m[3].raw as f32,
        ];

        let mut win: usize = 0;
        for (i, v) in vals.iter().enumerate() {
            if *v > vals[win] {
                win = i;
            }
        }
        let second: f32 = vals
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != win)
            .map(|(_, v)| *v)
            .fold(0.0, f32::max);

        let detected: bool = vals[win] >= NOISE_GATE && vals[win] > second * DOMINANCE;
        if detected {
            *silent_ticks = 0;
            // Dirección opuesta al micro ganador (el aire empuja hacia el otro lado).
            let dir: Vector2<f32> = match win {
                0 => Vector2::new(0.0, -1.0), // adelante → empuja hacia atrás
                1 => Vector2::new(-1.0, 0.0), // derecha → empuja hacia la izquierda
                2 => Vector2::new(0.0, 1.0),  // atrás → empuja hacia adelante
                _ => Vector2::new(1.0, 0.0),  // izquierda → empuja hacia la derecha
            };
            // Curva sqrt: los soplidos suaves se notan (0.12 lineal → 0.35) y
            // solo los fuertes de verdad llegan a 1.0.
            let strength: f32 = ((vals[win] - NOISE_GATE) / (MAX_RAW - NOISE_GATE))
                .clamp(0.0, 1.0)
                .sqrt();
            *wind_target = dir * strength;
        } else if vals[win] < NOISE_GATE {
            *silent_ticks += 1;
            if *silent_ticks >= SILENT_HOLD {
                // Silencio sostenido: el viento se apaga de verdad.
                *wind_target = Vector2::new(0.0, 0.0);
            }
        }
        // Ambiguo (ganador sin dominancia clara): conservar el objetivo anterior.

        const MIC_NAMES: [&str; 4] = ["adelante", "derecha", "atras", "izquierda"];
        println!(
            "[sensors] lux={:.1} mics=[{},{},{},{}] soplado={} wind_target=({:.2},{:.2})",
            light.lux,
            m[0].raw,
            m[1].raw,
            m[2].raw,
            m[3].raw,
            if detected { MIC_NAMES[win] } else { "-" },
            wind_target.x,
            wind_target.y
        );
    }

    // Suavizado del viento por frame (independiente de la cadencia de paquetes):
    // ataque rápido al soplar y caída lenta al parar, como una ráfaga real.
    let now: web_time::Instant = web_time::Instant::now();
    let dt: f32 = (now - *wind_last_t).as_secs_f32();
    *wind_last_t = now;
    let tc: f32 = if wind_target.magnitude() > wind_current.magnitude() {
        0.15 // ataque
    } else {
        0.6 // caída
    };
    let factor: f32 = 1.0 - (-dt / tc).exp();
    *wind_current += (*wind_target - *wind_current) * factor;
    state.set_wind(
        [wind_current.x, wind_current.y],
        wind_current.magnitude().clamp(0.0, 1.0),
    );

    if let Some(light_id) = demo_models.get("venus") {
        // Opción B: posición ABSOLUTA proporcionada por la cámara de la RPi.
        state
            .light_handle()
            .set_object_light_position(state, light_id, *light_pos, *last_lux);
    } else {
        log::error!("Invalid ID");
    };
}

fn setup(state: &mut ferrum::State, demo_models: &Rc<RefCell<HashMap<&str, usize>>>) {
    let mut demo_models = demo_models.borrow_mut();

    let plant: ModelDesc = ModelDesc::new(
        "plant/plant.obj",
        vec![
            Instance::new(
                Vector3::new(0.0, 0.0, 0.0),
                Quaternion::from_angle_y(Deg(0.0)),
                Vector3::new(1.0, 1.0, 1.0),
            )
            .with_wind(1.0), // marca la planta como follaje (se mueve con el viento)
        ],
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
