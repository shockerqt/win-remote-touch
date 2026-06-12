use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::{Html, IntoResponse},
    routing::get,
    Router,
};
use serde::Deserialize;
use std::mem::zeroed;
use tokio::net::TcpListener;
use windows_sys::Win32::Foundation::{POINT, RECT};
use windows_sys::Win32::UI::Input::Pointer::{
    InitializeTouchInjection, InjectTouchInput, POINTER_FLAG_DOWN, POINTER_FLAG_INCONTACT,
    POINTER_FLAG_INRANGE, POINTER_FLAG_UP, POINTER_FLAG_UPDATE, POINTER_INFO, POINTER_TOUCH_INFO,
    TOUCH_FEEDBACK_DEFAULT,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetSystemMetrics, PT_TOUCH, SM_CXSCREEN, SM_CYSCREEN, TOUCH_FLAG_NONE, TOUCH_MASK_CONTACTAREA,
    TOUCH_MASK_PRESSURE,
};

#[derive(Deserialize, Debug)]
struct TouchEvent {
    id: u32,
    x: f32,
    y: f32,
    action: String,
}

#[tokio::main]
async fn main() {
    println!("Inicializando inyección táctil de Windows...");
    unsafe {
        let init_result = InitializeTouchInjection(10, TOUCH_FEEDBACK_DEFAULT);
        if init_result == 0 {
            println!("Nota: Error al inicializar InitializeTouchInjection. (Ignorar si se ejecuta en Linux para pruebas de compilación)");
        } else {
            println!("API Táctil Inicializada para 10 dedos.");
        }
    }

    let app = Router::new()
        .route("/", get(index_handler))
        .route("/ws", get(ws_handler));

    let listener = TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("Servidor Backend corriendo en http://0.0.0.0:3000");
    axum::serve(listener, app).await.unwrap();
}

async fn index_handler() -> Html<&'static str> {
    let html = include_str!("../index.html");
    Html(html)
}

async fn ws_handler(ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(handle_socket)
}

async fn handle_socket(mut socket: WebSocket) {
    while let Some(Ok(msg)) = socket.recv().await {
        if let Message::Text(text) = msg {
            if let Ok(event) = serde_json::from_str::<TouchEvent>(&text) {
                inject_touch_event(&event);
            }
        }
    }
}

fn inject_touch_event(event: &TouchEvent) {
    unsafe {
        let mut pointer_info: POINTER_INFO = zeroed();
        pointer_info.pointerType = PT_TOUCH;
        pointer_info.pointerId = event.id;

        // Fase 4: Escalado desde coordenadas relativas a la resolución real de la pantalla
        let screen_width = GetSystemMetrics(SM_CXSCREEN) as f32;
        let screen_height = GetSystemMetrics(SM_CYSCREEN) as f32;

        let mapped_x = (event.x * screen_width) as i32;
        let mapped_y = (event.y * screen_height) as i32;

        pointer_info.ptPixelLocation = POINT {
            x: mapped_x,
            y: mapped_y,
        };

        let flags = match event.action.as_str() {
            "DOWN" => POINTER_FLAG_DOWN | POINTER_FLAG_INRANGE | POINTER_FLAG_INCONTACT,
            "MOVE" => POINTER_FLAG_UPDATE | POINTER_FLAG_INRANGE | POINTER_FLAG_INCONTACT,
            "UP" => POINTER_FLAG_UP,
            _ => 0,
        };
        pointer_info.pointerFlags = flags;

        let mut touch_info: POINTER_TOUCH_INFO = zeroed();
        touch_info.pointerInfo = pointer_info;
        touch_info.touchFlags = TOUCH_FLAG_NONE;
        touch_info.touchMask = TOUCH_MASK_CONTACTAREA | TOUCH_MASK_PRESSURE;

        let cx = mapped_x;
        let cy = mapped_y;
        touch_info.rcContact = RECT {
            left: cx - 10,
            top: cy - 10,
            right: cx + 10,
            bottom: cy + 10,
        };
        touch_info.pressure = 1024;

        InjectTouchInput(1, &touch_info);
    }
}
