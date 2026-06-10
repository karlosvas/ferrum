use {
    crate::{
        App,
        config::AppConfig,
        ui::{UiControls, control_panel},
    },
    cgmath::{InnerSpace, Vector2},
    ferrum::{
        Deg, Instance, Quaternion, Rotation3, TypeModel, Vector3,
        assets::{ModelDesc, models},
    },
    shared::structs::RpiDemo,
    std::{cell::RefCell, collections::HashMap, rc::Rc, sync::mpsc},
    tsl2591_rs::driver::SensorReading,
};

pub fn build_app(rx: mpsc::Receiver<RpiDemo>) -> App {
    let demo_models: Rc<RefCell<HashMap<&str, usize>>> = Rc::new(RefCell::new(HashMap::new()));
    let demo_models_update: Rc<RefCell<HashMap<&str, usize>>> = demo_models.clone();

    let app_config: AppConfig = AppConfig::new(Some(ferrum::PhysicalSize::new(1000, 500)));

    let mut last_lux: f32 = 0.0;
    let mut light_pos: Vector3<f32> = Vector3::new(0.0, 0.0, 0.0);
    let mut wind_target: Vector2<f32> = Vector2::new(0.0, 0.0);
    let mut wind_current: Vector2<f32> = Vector2::new(0.0, 0.0);
    let mut wind_last_t: web_time::Instant = web_time::Instant::now();
    let mut silent_ticks: u32 = 0;

    let mut last_packet: Option<web_time::Instant> = None;
    let mut conn_started: web_time::Instant = web_time::Instant::now();
    let mut conn_attempts: u32 = 0;

    let controls: Rc<RefCell<UiControls>> = Rc::new(RefCell::new(UiControls::default()));
    let controls_ui: Rc<RefCell<UiControls>> = Rc::clone(&controls);

    App::new(app_config)
        .ferrum_setup(move |state| setup(state, &demo_models))
        .ferrum_ui(move |ctx| control_panel(ctx, &mut controls_ui.borrow_mut()))
        .ferrum_update(move |state| {
            update(
                state,
                &demo_models_update,
                &rx,
                &controls,
                &mut last_lux,
                &mut light_pos,
                &mut wind_target,
                &mut wind_current,
                &mut wind_last_t,
                &mut silent_ticks,
                &mut last_packet,
                &mut conn_started,
                &mut conn_attempts,
            )
        })
}

#[allow(clippy::too_many_arguments)]
fn update(
    state: &mut ferrum::State,
    demo_models: &Rc<RefCell<HashMap<&str, usize>>>,
    rx: &mpsc::Receiver<RpiDemo>,
    controls: &Rc<RefCell<UiControls>>,
    last_lux: &mut f32,
    light_pos: &mut Vector3<f32>,
    wind_target: &mut Vector2<f32>,
    wind_current: &mut Vector2<f32>,
    wind_last_t: &mut web_time::Instant,
    silent_ticks: &mut u32,
    last_packet: &mut Option<web_time::Instant>,
    conn_started: &mut web_time::Instant,
    conn_attempts: &mut u32,
) {
    let demo_models = demo_models.borrow_mut();
    let mut controls = controls.borrow_mut();

    state.last_render_time = web_time::Instant::now();

    // (Re)connection request from the panel
    if controls.ssh_requested {
        controls.ssh_requested = false;
        *conn_attempts = 0;
        *conn_started = web_time::Instant::now();
        #[cfg(not(target_arch = "wasm32"))]
        crate::ssh::spawn_connect(
            controls.rpi_user.clone(),
            controls.rpi_host.clone(),
            controls.ip_host.clone(),
            std::sync::Arc::clone(&controls.ssh_status),
        );
    }

    // Consume all pending data and keep the most recent. ALWAYS drained (even
    // with RPi mode off) to know whether the Pi is alive and to keep the
    // channel from piling up old packets.
    while let Ok(new_data) = rx.try_recv() {
        *last_packet = Some(web_time::Instant::now());
        if !controls.rpi_mode {
            continue;
        }
        let light: SensorReading = new_data.light;
        *last_lux = light.lux;
        *light_pos = Vector3::new(
            new_data.camera.x as f32,
            new_data.camera.y as f32,
            new_data.camera.z as f32,
        );

        // Wind from the 4 microphones. The RPi already sends each channel's
        // activity above its noise floor:
        //   channel 1=front, 2=right, 3=back, 4=left  →  indices 0..3.
        //
        // Blowing is acoustically noisy: the blown mic saturates (~32000) but
        // its neighbors also rise (2000-6000), so a differential subtraction
        // yields chaotic directions. Instead: WINNER-TAKE-ALL with dominance.
        // The direction only changes if one mic clearly beats the runner-up; if
        // the reading is ambiguous the previous wind is kept (no swerving).
        //
        // Air travels FROM the winning mic TOWARDS the plant: blowing the right
        // one pushes the leaves to the left. If it looks inverted on screen
        // (depends on your scene orientation), flip the signs.
        const NOISE_GATE: f32 = 1500.0; // below this, silence
        const DOMINANCE: f32 = 1.5; // the winner must beat the 2nd by this factor
        // Amplitude at which the sway is maximal. A strong blow saturates the
        // ADC at ~33000; it used to be 8000 and ANY blow reached the maximum —
        // which is why strong and soft looked equally intense.
        const MAX_RAW: f32 = 30000.0;
        // Blowing is not continuous: between breaths there are 1-3 packets at
        // zero. Instead of killing the wind at the first silence, the last
        // target is held for a few packets so the gust doesn't flicker.
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
            // Direction opposite to the winning mic (air pushes towards the other side).
            let dir: Vector2<f32> = match win {
                0 => Vector2::new(0.0, -1.0), // front → pushes backwards
                1 => Vector2::new(-1.0, 0.0), // right → pushes to the left
                2 => Vector2::new(0.0, 1.0),  // back → pushes forwards
                _ => Vector2::new(1.0, 0.0),  // left → pushes to the right
            };
            // Sqrt curve: soft blows are noticeable (0.12 linear → 0.35) and
            // only truly strong ones reach 1.0.
            let strength: f32 = ((vals[win] - NOISE_GATE) / (MAX_RAW - NOISE_GATE))
                .clamp(0.0, 1.0)
                .sqrt();
            *wind_target = dir * strength;
        } else if vals[win] < NOISE_GATE {
            *silent_ticks += 1;
            if *silent_ticks >= SILENT_HOLD {
                // Sustained silence: the wind actually turns off.
                *wind_target = Vector2::new(0.0, 0.0);
            }
        }
        // Ambiguous (winner without clear dominance): keep the previous target.

        const MIC_NAMES: [&str; 4] = ["front", "right", "back", "left"];
        log::info!(
            "[sensors] lux={:.1} mics=[{},{},{},{}] blown={} wind_target=({:.2},{:.2})",
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

    // --- RPi connection state ---
    // With no packet at all after 3 windows of 3s, it switches to manual mode
    // (sliders) on its own; the user can re-enable RPi mode from the panel.
    const RETRY_WINDOW_SECS: f32 = 3.0;
    const MAX_ATTEMPTS: u32 = 3;
    let now: web_time::Instant = web_time::Instant::now();
    match *last_packet {
        Some(t) => {
            // Connected if there is data within the last 5s.
            controls.rpi_connected = (now - t).as_secs_f32() < 5.0;
        }
        None => {
            controls.rpi_connected = false;
            if *conn_attempts < MAX_ATTEMPTS
                && (now - *conn_started).as_secs_f32() > RETRY_WINDOW_SECS
            {
                *conn_attempts += 1;
                *conn_started = now;
                if *conn_attempts >= MAX_ATTEMPTS {
                    if controls.rpi_mode {
                        controls.rpi_mode = false;
                        log::info!(
                            "[connection] No data from RPi after {MAX_ATTEMPTS} attempts; manual mode (sliders)"
                        );
                    }
                } else {
                    log::info!(
                        "[connection] waiting for RPi data (attempt {}/{})",
                        conn_attempts,
                        MAX_ATTEMPTS
                    );
                }
            }
        }
    }

    // --- Manual mode: sliders replace the sensors ---
    if !controls.rpi_mode {
        // Venus orbits the plant at fixed radius/height per the slider.
        const ORBIT_RADIUS: f32 = 8.0;
        const ORBIT_HEIGHT: f32 = 5.0;
        let angle: f32 = controls.orbit_angle.to_radians();
        *light_pos = Vector3::new(
            ORBIT_RADIUS * angle.cos(),
            ORBIT_HEIGHT,
            ORBIT_RADIUS * angle.sin(),
        );
        *last_lux = controls.light_lux;

        // Same convention as the mics: each slider is the wind's ORIGIN,
        // and the air pushes the leaves towards the opposite side.
        let w: [f32; 4] = controls.wind;
        let manual: Vector2<f32> = Vector2::new(0.0, -1.0) * w[0]   // front
            + Vector2::new(-1.0, 0.0) * w[1]                        // right
            + Vector2::new(0.0, 1.0) * w[2]                         // back
            + Vector2::new(1.0, 0.0) * w[3]; // left
        *wind_target = if manual.magnitude() > 1.0 {
            manual.normalize()
        } else {
            manual
        };
    }

    // Per-frame wind smoothing (independent of the packet cadence):
    // fast attack when blowing and slow decay when stopping, like a real gust.
    let now: web_time::Instant = web_time::Instant::now();
    let dt: f32 = (now - *wind_last_t).as_secs_f32();
    *wind_last_t = now;
    let tc: f32 = if wind_target.magnitude() > wind_current.magnitude() {
        0.15 // attack
    } else {
        0.6 // decay
    };
    let factor: f32 = 1.0 - (-dt / tc).exp();
    *wind_current += (*wind_target - *wind_current) * factor;
    state.set_wind(
        [wind_current.x, wind_current.y],
        wind_current.magnitude().clamp(0.0, 1.0),
    );

    if let Some(light_id) = demo_models.get("venus") {
        // Option B: ABSOLUTE position provided by the RPi camera.
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
            .with_wind(1.0), // marks the plant as foliage (moves with the wind)
        ],
        TypeModel::StaticObj,
    );

    let ingot: ferrum::Ingot<models::Model> = state.spawn_model(plant);
    demo_models.insert("plant", ingot.id);

    let floor: ModelDesc = ModelDesc::new(
        "floor/floor.obj",
        vec![Instance::default()],
        TypeModel::StaticObj,
    );

    let ingot: ferrum::Ingot<models::Model> = state.spawn_model(floor);
    demo_models.insert("floor", ingot.id);

    let venus: ModelDesc = ModelDesc::new(
        "sun/venus.obj",
        vec![Instance::default()],
        TypeModel::PointOfLight,
    );

    let ingot: ferrum::Ingot<models::Model> = state.spawn_model(venus);
    demo_models.insert("venus", ingot.id);
}
