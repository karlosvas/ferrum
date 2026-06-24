use std::{
    net::UdpSocket,
    path::Path,
    process::Command,
    sync::{Arc, Mutex},
};

pub fn local_ip() -> Option<String> {
    let socket: UdpSocket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    Some(socket.local_addr().ok()?.ip().to_string())
}

pub fn spawn_connect(user: String, host: String, ip_host: String, status: Arc<Mutex<String>>) {
    std::thread::spawn(move || {
        let set = |msg: String| {
            log::info!("[ssh] {msg}");
            if let Ok(mut s) = status.lock() {
                *s = msg;
            }
        };
        if user.is_empty() || host.is_empty() {
            set("need users or host of RPi".to_string());
            return;
        }
        let target: String = format!("{user}@{host}");

        const BIN: &str = "target/aarch64-unknown-linux-gnu/release/rpi";
        if Path::new(BIN).exists() {
            set("coping binary (scp)…".to_string());
            match Command::new("scp")
                .args(["-o", "BatchMode=yes", BIN, &format!("{target}:~/rpi")])
                .output()
            {
                Ok(o) if o.status.success() => {}
                Ok(o) => {
                    set(format!(
                        "scp failed: {}",
                        String::from_utf8_lossy(&o.stderr).trim()
                    ));
                    return;
                }
                Err(e) => {
                    set(format!("scp not aviable: {e}"));
                    return;
                }
            }
        } else {
            set("sin binario local (cargo xtask rpi); uso el ya desplegado".to_string());
        }

        let remote: String = format!(
            "pkill -x rpi 2>/dev/null; chmod +x ~/rpi && IP_HOST={ip_host} nohup ~/rpi >/dev/null 2>&1 & exit 0"
        );
        set(format!("conectando a {target}…"));
        match Command::new("ssh")
            .args([
                "-o",
                "BatchMode=yes",
                "-o",
                "ConnectTimeout=5",
                &target,
                &remote,
            ])
            .output()
        {
            Ok(o) if o.status.success() => set(format!("✓ rpi launched in {host}")),
            Ok(o) => set(format!(
                "ssh failed: {}",
                String::from_utf8_lossy(&o.stderr).trim()
            )),
            Err(e) => set(format!("ssh not aviable: {e}")),
        }
    });
}
