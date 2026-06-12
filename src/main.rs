use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::{Html, IntoResponse},
    routing::get,
    Router,
};
use serde::Deserialize;
use std::mem::{size_of, zeroed};
use tokio::net::TcpListener;

use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_MOUSE, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP,
    MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP, MOUSEEVENTF_MOVE, MOUSEEVENTF_RIGHTDOWN,
    MOUSEEVENTF_RIGHTUP, MOUSEEVENTF_WHEEL,
};

#[derive(Deserialize, Debug, PartialEq)]
#[serde(tag = "action")]
enum TouchpadEvent {
    MOVE { dx: f32, dy: f32 },
    SCROLL { dy: f32 },
    CLICK { button: String },
}

#[tokio::main]
async fn main() {
    println!("Inicializando Servidor de Touchpad Remoto...");

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
            if let Ok(event) = serde_json::from_str::<TouchpadEvent>(&text) {
                println!("Acción recibida: {:?}", event);
                process_event(&event);
            }
        }
    }
}

fn process_event(event: &TouchpadEvent) {
    unsafe {
        match event {
            TouchpadEvent::MOVE { dx, dy } => {
                let mut input: INPUT = zeroed();
                input.r#type = INPUT_MOUSE;

                // Accelerate movement slightly for better touchpad feel
                let accel = 1.5;
                input.Anonymous.mi.dx = (*dx * accel) as i32;
                input.Anonymous.mi.dy = (*dy * accel) as i32;
                input.Anonymous.mi.dwFlags = MOUSEEVENTF_MOVE;

                SendInput(1, &input, size_of::<INPUT>() as i32);
            }
            TouchpadEvent::SCROLL { dy } => {
                let mut input: INPUT = zeroed();
                input.r#type = INPUT_MOUSE;

                // Windows WHEEL_DELTA is 120. A JS swipe up gives a negative dy.
                // Scroll down in Windows expects a negative mouseData.
                let scroll_amount = (*dy * -2.0) as i32; // invert and scale
                input.Anonymous.mi.mouseData = scroll_amount as u32;
                input.Anonymous.mi.dwFlags = MOUSEEVENTF_WHEEL;

                SendInput(1, &input, size_of::<INPUT>() as i32);
            }
            TouchpadEvent::CLICK { button } => {
                let (down, up) = match button.as_str() {
                    "LEFT" => (MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP),
                    "RIGHT" => (MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP),
                    "MIDDLE" => (MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP),
                    _ => return,
                };

                let mut inputs: [INPUT; 2] = [zeroed(), zeroed()];

                inputs[0].r#type = INPUT_MOUSE;
                inputs[0].Anonymous.mi.dwFlags = down;

                inputs[1].r#type = INPUT_MOUSE;
                inputs[1].Anonymous.mi.dwFlags = up;

                SendInput(2, inputs.as_ptr(), size_of::<INPUT>() as i32);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_move() {
        let json = r#"{"action": "MOVE", "dx": 15.5, "dy": -3.2}"#;
        let event: TouchpadEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event, TouchpadEvent::MOVE { dx: 15.5, dy: -3.2 });
    }

    #[test]
    fn test_parse_scroll() {
        let json = r#"{"action": "SCROLL", "dx": 0, "dy": 20.0}"#; // frontend still sends dx though ignored
        let event: TouchpadEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event, TouchpadEvent::SCROLL { dy: 20.0 });
    }

    #[test]
    fn test_parse_click_left() {
        let json = r#"{"action": "CLICK", "button": "LEFT"}"#;
        let event: TouchpadEvent = serde_json::from_str(json).unwrap();
        assert_eq!(
            event,
            TouchpadEvent::CLICK {
                button: "LEFT".to_string()
            }
        );
    }

    #[test]
    fn test_parse_click_right() {
        let json = r#"{"action": "CLICK", "button": "RIGHT"}"#;
        let event: TouchpadEvent = serde_json::from_str(json).unwrap();
        assert_eq!(
            event,
            TouchpadEvent::CLICK {
                button: "RIGHT".to_string()
            }
        );
    }
}
