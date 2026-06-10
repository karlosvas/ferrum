use {
    ferrum::State,
    std::sync::{Arc, Mutex},
    winit::window::Window,
};

/// Estado compartido entre el panel de egui y el `update` del demo.
/// La UI escribe aquí (toggle + sliders) y el update decide si manda al motor
/// los datos reales de la RPi o los valores manuales de los sliders.
pub struct UiControls {
    /// true = usar los sensores de la RPi; false = modo manual con sliders.
    pub rpi_mode: bool,
    /// Solo informativo: si estamos recibiendo paquetes de la RPi.
    pub rpi_connected: bool,
    /// Ángulo de la órbita de Venus alrededor de la planta (grados).
    pub orbit_angle: f32,
    /// Intensidad de luz manual, en lux (misma escala que el sensor TSL2591).
    pub light_lux: f32,
    /// Viento manual por dirección de origen: adelante, derecha, atrás, izquierda.
    pub wind: [f32; 4],
    /// Usuario SSH de la RPi (editable; precargado de RPI_USER).
    pub rpi_user: String,
    /// IP/host de la RPi (editable; precargado de RPI_HOST).
    pub rpi_host: String,
    /// IP de ESTA máquina, a la que la Pi conecta de vuelta (IP_HOST).
    pub ip_host: String,
    /// El botón "Conectar" la pone a true; el update nativo la consume y
    /// lanza el despliegue SSH en segundo plano.
    pub ssh_requested: bool,
    /// Última línea de estado del despliegue SSH (la escribe el hilo de ssh).
    pub ssh_status: Arc<Mutex<String>>,
}

impl Default for UiControls {
    fn default() -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        let (rpi_user, rpi_host, ip_host) = (
            std::env::var("RPI_USER").unwrap_or_default(),
            std::env::var("RPI_HOST").unwrap_or_default(),
            std::env::var("IP_HOST")
                .ok()
                .or_else(crate::ssh::local_ip)
                .unwrap_or_else(|| "127.0.0.1".to_string()),
        );
        #[cfg(target_arch = "wasm32")]
        let (rpi_user, rpi_host, ip_host) = (String::new(), String::new(), String::new());

        Self {
            rpi_mode: true,
            rpi_connected: false,
            orbit_angle: 0.0,
            light_lux: 400.0,
            wind: [0.0; 4],
            rpi_user,
            rpi_host,
            ip_host,
            ssh_requested: false,
            ssh_status: Arc::new(Mutex::new(String::new())),
        }
    }
}

/// Construye el panel de control anclado arriba a la derecha.
pub fn control_panel(ctx: &egui::Context, c: &mut UiControls) {
    egui::Window::new("Control")
        .anchor(egui::Align2::RIGHT_TOP, [-10.0, 10.0])
        .resizable(false)
        .show(ctx, |ui| {
            // Reactivar el modo RPi sin datos relanza el despliegue SSH solo.
            if ui.checkbox(&mut c.rpi_mode, "Conexión RPi").changed()
                && c.rpi_mode
                && !c.rpi_connected
            {
                c.ssh_requested = true;
            }
            ui.label(if c.rpi_connected {
                egui::RichText::new("● recibiendo datos").color(egui::Color32::GREEN)
            } else {
                egui::RichText::new("● sin datos").color(egui::Color32::RED)
            });

            // Despliegue por SSH (solo nativo: en la web no hay ssh).
            #[cfg(not(target_arch = "wasm32"))]
            if c.rpi_mode {
                ui.separator();
                ui.label("RPi por SSH");
                egui::Grid::new("ssh_grid").num_columns(2).show(ui, |ui| {
                    ui.label("Usuario");
                    ui.text_edit_singleline(&mut c.rpi_user);
                    ui.end_row();
                    ui.label("Host/IP");
                    ui.text_edit_singleline(&mut c.rpi_host);
                    ui.end_row();
                    ui.label("IP demo");
                    ui.text_edit_singleline(&mut c.ip_host);
                    ui.end_row();
                });
                if ui.button("Conectar (scp + ssh)").clicked() {
                    c.ssh_requested = true;
                }
                if let Ok(status) = c.ssh_status.lock()
                    && !status.is_empty()
                {
                    ui.label(egui::RichText::new(status.as_str()).small());
                }
            }
            ui.separator();

            // Los sliders solo mandan cuando el modo RPi está apagado.
            ui.add_enabled_ui(!c.rpi_mode, |ui| {
                ui.label("Venus (órbita)");
                ui.add(egui::Slider::new(&mut c.orbit_angle, 0.0..=360.0).suffix("°"));
                ui.label("Luz");
                ui.add(egui::Slider::new(&mut c.light_lux, 0.0..=1000.0).suffix(" lux"));
                ui.separator();
                ui.label("Viento");
                const NAMES: [&str; 4] = ["Adelante", "Derecha", "Atrás", "Izquierda"];
                for (value, name) in c.wind.iter_mut().zip(NAMES) {
                    ui.add(egui::Slider::new(value, 0.0..=1.0).text(name));
                }
            });
        });
}

/// Integración de egui sobre el render de ferrum: traduce eventos de winit,
/// ejecuta la UI y la pinta como pasada extra sobre el frame final.
pub struct EguiLayer {
    pub ctx: egui::Context,
    winit_state: egui_winit::State,
    renderer: egui_wgpu::Renderer,
}

impl EguiLayer {
    pub fn new(state: &State, window: &Window) -> Self {
        let ctx: egui::Context = egui::Context::default();
        let winit_state: egui_winit::State = egui_winit::State::new(
            ctx.clone(),
            egui::ViewportId::ROOT,
            window,
            Some(window.scale_factor() as f32),
            None,
            None,
        );
        // egui pinta sobre la vista final del frame (post-tonemapping), que se
        // crea con el formato sRGB de la superficie.
        let renderer: egui_wgpu::Renderer = egui_wgpu::Renderer::new(
            &state.device,
            state.config.format.add_srgb_suffix(),
            egui_wgpu::RendererOptions::default(),
        );
        Self {
            ctx,
            winit_state,
            renderer,
        }
    }

    /// Devuelve true si egui consumió el evento (p. ej. arrastrar un slider);
    /// en ese caso no debe llegar a la cámara del motor.
    pub fn on_window_event(&mut self, window: &Window, event: &winit::event::WindowEvent) -> bool {
        self.winit_state.on_window_event(window, event).consumed
    }

    /// Renderiza el frame con la UI superpuesta. Sustituye a `state.render()`.
    pub fn render_with_ui(
        &mut self,
        state: &mut State,
        window: &Arc<Window>,
        ui_fn: &mut dyn FnMut(&egui::Context),
    ) -> Result<(), ferrum::SurfaceError> {
        let raw_input: egui::RawInput = self.winit_state.take_egui_input(window);
        let output: egui::FullOutput = self.ctx.run_ui(raw_input, |ui| ui_fn(ui.ctx()));
        self.winit_state
            .handle_platform_output(window, output.platform_output);

        let pixels_per_point: f32 = output.pixels_per_point;
        let tris: Vec<egui::ClippedPrimitive> = self.ctx.tessellate(output.shapes, pixels_per_point);
        let screen: egui_wgpu::ScreenDescriptor = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [state.config.width, state.config.height],
            pixels_per_point,
        };

        let renderer: &mut egui_wgpu::Renderer = &mut self.renderer;
        let textures_delta: &egui::TexturesDelta = &output.textures_delta;
        state.render_with_overlay(&mut |device, queue, encoder, view| {
            for (id, delta) in &textures_delta.set {
                renderer.update_texture(device, queue, *id, delta);
            }
            renderer.update_buffers(device, queue, encoder, &tris, &screen);

            // load: Load — la escena ya está pintada; egui solo se superpone.
            let mut pass: wgpu::RenderPass<'static> = encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("egui_pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                })
                .forget_lifetime();
            renderer.render(&mut pass, &tris, &screen);
            drop(pass);

            for id in &textures_delta.free {
                renderer.free_texture(id);
            }
        })
    }
}
