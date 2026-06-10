//! Despliegue/arranque remoto del binario `rpi` por SSH, la misma receta que
//! `cargo xtask demo` (scp del binario cross-compilado + ssh para lanzarlo),
//! pero disparado desde el panel de la demo y sin bloquear el render loop.

use std::{
    net::UdpSocket,
    path::Path,
    process::Command,
    sync::{Arc, Mutex},
};

/// IP local con la que esta máquina sale a la red; es la que la RPi usará
/// para conectarse de vuelta al servidor de websockets de la demo.
/// (El connect de UDP no manda ningún paquete, solo fija la ruta.)
pub fn local_ip() -> Option<String> {
    let socket: UdpSocket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    Some(socket.local_addr().ok()?.ip().to_string())
}

/// Lanza en un hilo aparte: scp del binario (si existe localmente) + ssh para
/// arrancarlo en la Pi con IP_HOST apuntando a esta máquina. El progreso se
/// publica en `status` para que el panel lo muestre.
pub fn spawn_connect(user: String, host: String, ip_host: String, status: Arc<Mutex<String>>) {
    std::thread::spawn(move || {
        let set = |msg: String| {
            log::info!("[ssh] {msg}");
            if let Ok(mut s) = status.lock() {
                *s = msg;
            }
        };
        if user.is_empty() || host.is_empty() {
            set("falta usuario u host de la RPi".to_string());
            return;
        }
        let target: String = format!("{user}@{host}");

        // El binario que produce `cargo xtask rpi` (cross). Si no está, se
        // intenta arrancar el que ya haya desplegado en la Pi.
        const BIN: &str = "target/aarch64-unknown-linux-gnu/release/rpi";
        if Path::new(BIN).exists() {
            set("copiando binario (scp)…".to_string());
            match Command::new("scp")
                .args(["-o", "BatchMode=yes", BIN, &format!("{target}:~/rpi")])
                .output()
            {
                Ok(o) if o.status.success() => {}
                Ok(o) => {
                    set(format!(
                        "scp falló: {}",
                        String::from_utf8_lossy(&o.stderr).trim()
                    ));
                    return;
                }
                Err(e) => {
                    set(format!("scp no disponible: {e}"));
                    return;
                }
            }
        } else {
            set("sin binario local (cargo xtask rpi); uso el ya desplegado".to_string());
        }

        // Misma receta que xtask::connect_rpi, con nohup para que el proceso
        // sobreviva al cierre de la sesión y pkill para no acumular instancias.
        // BatchMode evita que ssh se quede colgado pidiendo contraseña (hace
        // falta clave pública instalada en la Pi).
        let remote: String = format!(
            "pkill -x rpi 2>/dev/null; chmod +x ~/rpi && IP_HOST={ip_host} nohup ~/rpi >/dev/null 2>&1 & exit 0"
        );
        set(format!("conectando a {target}…"));
        match Command::new("ssh")
            .args(["-o", "BatchMode=yes", "-o", "ConnectTimeout=5", &target, &remote])
            .output()
        {
            Ok(o) if o.status.success() => set(format!("✓ rpi lanzado en {host}")),
            Ok(o) => set(format!(
                "ssh falló: {}",
                String::from_utf8_lossy(&o.stderr).trim()
            )),
            Err(e) => set(format!("ssh no disponible: {e}")),
        }
    });
}
