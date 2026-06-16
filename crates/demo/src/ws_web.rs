use {
    shared::structs::RpiDemo,
    std::sync::mpsc::Sender,
    wasm_bindgen::{JsCast, closure::Closure},
    web_sys::{BinaryType, CloseEvent, ErrorEvent, MessageEvent, WebSocket},
};

const DEMO_WS_PORT: u16 = 3000;

pub fn connect(tx: Sender<RpiDemo>) {
    let host: String = web_sys::window()
        .and_then(|w| w.location().hostname().ok())
        .filter(|h| !h.is_empty())
        .unwrap_or_else(|| "localhost".into());
    let url: String = format!("ws://{host}:{DEMO_WS_PORT}/demo");

    let ws: WebSocket = match WebSocket::new(&url) {
        Ok(ws) => ws,
        Err(e) => {
            log::warn!("No se pudo abrir el WebSocket {url} ({e:?}); modo manual");
            return;
        }
    };
    ws.set_binary_type(BinaryType::Arraybuffer);

    let onmessage = Closure::<dyn FnMut(MessageEvent)>::new(move |e: MessageEvent| {
        let Ok(buffer) = e.data().dyn_into::<js_sys::ArrayBuffer>() else {
            return;
        };
        let bytes: Vec<u8> = js_sys::Uint8Array::new(&buffer).to_vec();
        match bincode::serde::decode_from_slice::<RpiDemo, _>(&bytes, bincode::config::standard()) {
            Ok((data, _)) => {
                let _ = tx.send(data);
            }
            Err(e) => log::warn!("Paquete RpiDemo inválido: {e}"),
        }
    });
    ws.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
    onmessage.forget();

    let onerror = Closure::<dyn FnMut(ErrorEvent)>::new(move |_: ErrorEvent| {
        log::warn!("Error en el WebSocket; la demo sigue en modo manual");
    });
    ws.set_onerror(Some(onerror.as_ref().unchecked_ref()));
    onerror.forget();

    let onclose = Closure::<dyn FnMut(CloseEvent)>::new(move |e: CloseEvent| {
        log::info!("WebSocket cerrado (code={})", e.code());
    });
    ws.set_onclose(Some(onclose.as_ref().unchecked_ref()));
    onclose.forget();
}
