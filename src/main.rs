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
    SendInput, INPUT, INPUT_KEYBOARD, INPUT_MOUSE, KEYEVENTF_KEYUP, MOUSEEVENTF_HWHEEL,
    MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP,
    MOUSEEVENTF_MOVE, MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP, MOUSEEVENTF_WHEEL, VK_D,
    VK_LCONTROL, VK_LEFT, VK_LWIN, VK_RIGHT, VK_TAB,
};

#[derive(Deserialize, Debug, PartialEq)]
#[serde(tag = "action", rename_all = "UPPERCASE")]
enum TouchpadEvent {
    Move {
        dx: f32,
        dy: f32,
    },
    DragStart,
    DragMove {
        dx: f32,
        dy: f32,
    },
    DragEnd,
    Scroll {
        dx: f32,
        dy: f32,
        ctrl: bool,
    },
    Click {
        button: String,
    },
    Swipe {
        direction: String,
    },
    Configure {
        cursor_speed: f32,
        scroll_speed: f32,
    },
}

#[derive(Debug)]
struct SessionState {
    cursor_speed: f32,
    scroll_speed: f32,
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
    let mut state = SessionState {
        cursor_speed: 1.0,
        scroll_speed: 1.0,
    };
    while let Some(Ok(msg)) = socket.recv().await {
        if let Message::Text(text) = msg {
            if let Ok(event) = serde_json::from_str::<TouchpadEvent>(&text) {
                println!("Acción recibida: {:?}", event);
                process_event(&mut state, &event).await;
            }
        }
    }
}

fn move_mouse(dx: f32, dy: f32, cursor_speed: f32) {
    let dist = (dx * dx + dy * dy).sqrt();
    let curve_factor = if dist < 2.0 {
        1.0
    } else if dist < 10.0 {
        1.5 + (dist - 2.0) * 0.15
    } else {
        2.7 + (dist - 10.0) * 0.25
    };
    unsafe {
        let mut input: INPUT = zeroed();
        input.r#type = INPUT_MOUSE;
        input.Anonymous.mi.dx = (dx * curve_factor * cursor_speed) as i32;
        input.Anonymous.mi.dy = (dy * curve_factor * cursor_speed) as i32;
        input.Anonymous.mi.dwFlags = MOUSEEVENTF_MOVE;
        SendInput(1, &input, size_of::<INPUT>() as i32);
    }
}

unsafe fn send_shortcut(mods: &[u16], key: u16) {
    let mut inputs = Vec::new();

    // Press modifiers down
    for &m in mods {
        let mut input: INPUT = zeroed();
        input.r#type = INPUT_KEYBOARD;
        input.Anonymous.ki.wVk = m;
        input.Anonymous.ki.dwFlags = 0;
        inputs.push(input);
    }

    // Press main key down
    {
        let mut input: INPUT = zeroed();
        input.r#type = INPUT_KEYBOARD;
        input.Anonymous.ki.wVk = key;
        input.Anonymous.ki.dwFlags = 0;
        inputs.push(input);
    }

    // Release main key up
    {
        let mut input: INPUT = zeroed();
        input.r#type = INPUT_KEYBOARD;
        input.Anonymous.ki.wVk = key;
        input.Anonymous.ki.dwFlags = KEYEVENTF_KEYUP;
        inputs.push(input);
    }

    // Release modifiers up in reverse order
    for &m in mods.iter().rev() {
        let mut input: INPUT = zeroed();
        input.r#type = INPUT_KEYBOARD;
        input.Anonymous.ki.wVk = m;
        input.Anonymous.ki.dwFlags = KEYEVENTF_KEYUP;
        inputs.push(input);
    }

    SendInput(
        inputs.len() as u32,
        inputs.as_ptr(),
        size_of::<INPUT>() as i32,
    );
}

async fn process_event(state: &mut SessionState, event: &TouchpadEvent) {
    unsafe {
        match event {
            TouchpadEvent::Move { dx, dy } => {
                move_mouse(*dx, *dy, state.cursor_speed);
            }
            TouchpadEvent::DragStart => {
                let mut input: INPUT = zeroed();
                input.r#type = INPUT_MOUSE;
                input.Anonymous.mi.dwFlags = MOUSEEVENTF_LEFTDOWN;
                SendInput(1, &input, size_of::<INPUT>() as i32);
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }
            TouchpadEvent::DragMove { dx, dy } => {
                move_mouse(*dx, *dy, state.cursor_speed);
            }
            TouchpadEvent::DragEnd => {
                let mut input: INPUT = zeroed();
                input.r#type = INPUT_MOUSE;
                input.Anonymous.mi.dwFlags = MOUSEEVENTF_LEFTUP;
                SendInput(1, &input, size_of::<INPUT>() as i32);
            }
            TouchpadEvent::Scroll { dx, dy, ctrl } => {
                let scale = -2.0;
                if *ctrl {
                    let mut inputs: [INPUT; 3] = [zeroed(), zeroed(), zeroed()];

                    // 1. Ctrl down
                    inputs[0].r#type = INPUT_KEYBOARD;
                    inputs[0].Anonymous.ki.wVk = VK_LCONTROL;
                    inputs[0].Anonymous.ki.dwFlags = 0;

                    // 2. Mouse wheel
                    let scroll_amount = (*dy * state.scroll_speed * scale) as i32;
                    inputs[1].r#type = INPUT_MOUSE;
                    inputs[1].Anonymous.mi.mouseData = scroll_amount as u32;
                    inputs[1].Anonymous.mi.dwFlags = MOUSEEVENTF_WHEEL;

                    // 3. Ctrl up
                    inputs[2].r#type = INPUT_KEYBOARD;
                    inputs[2].Anonymous.ki.wVk = VK_LCONTROL;
                    inputs[2].Anonymous.ki.dwFlags = KEYEVENTF_KEYUP;

                    SendInput(3, inputs.as_ptr(), size_of::<INPUT>() as i32);
                } else {
                    let mut inputs = Vec::new();
                    if *dy != 0.0 {
                        let mut input: INPUT = zeroed();
                        input.r#type = INPUT_MOUSE;
                        let scroll_amount = (*dy * state.scroll_speed * scale) as i32;
                        input.Anonymous.mi.mouseData = scroll_amount as u32;
                        input.Anonymous.mi.dwFlags = MOUSEEVENTF_WHEEL;
                        inputs.push(input);
                    }
                    if *dx != 0.0 {
                        let mut input: INPUT = zeroed();
                        input.r#type = INPUT_MOUSE;
                        let scroll_amount = (*dx * state.scroll_speed * scale) as i32;
                        input.Anonymous.mi.mouseData = scroll_amount as u32;
                        input.Anonymous.mi.dwFlags = MOUSEEVENTF_HWHEEL;
                        inputs.push(input);
                    }
                    if !inputs.is_empty() {
                        SendInput(
                            inputs.len() as u32,
                            inputs.as_ptr(),
                            size_of::<INPUT>() as i32,
                        );
                    }
                }
            }
            TouchpadEvent::Click { button } => {
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
            TouchpadEvent::Swipe { direction } => match direction.as_str() {
                "UP" => send_shortcut(&[VK_LWIN], VK_TAB),
                "DOWN" => send_shortcut(&[VK_LWIN], VK_D),
                "LEFT" => send_shortcut(&[VK_LCONTROL, VK_LWIN], VK_LEFT),
                "RIGHT" => send_shortcut(&[VK_LCONTROL, VK_LWIN], VK_RIGHT),
                _ => {}
            },
            TouchpadEvent::Configure {
                cursor_speed,
                scroll_speed,
            } => {
                state.cursor_speed = *cursor_speed;
                state.scroll_speed = *scroll_speed;
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
        assert_eq!(event, TouchpadEvent::Move { dx: 15.5, dy: -3.2 });
    }

    #[test]
    fn test_parse_scroll() {
        let json = r#"{"action": "SCROLL", "dx": 0.0, "dy": 20.0, "ctrl": false}"#;
        let event: TouchpadEvent = serde_json::from_str(json).unwrap();
        assert_eq!(
            event,
            TouchpadEvent::Scroll {
                dx: 0.0,
                dy: 20.0,
                ctrl: false
            }
        );
    }

    #[test]
    fn test_parse_click_left() {
        let json = r#"{"action": "CLICK", "button": "LEFT"}"#;
        let event: TouchpadEvent = serde_json::from_str(json).unwrap();
        assert_eq!(
            event,
            TouchpadEvent::Click {
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
            TouchpadEvent::Click {
                button: "RIGHT".to_string()
            }
        );
    }

    #[test]
    fn test_parse_drag_start() {
        let json = r#"{"action": "DRAGSTART"}"#;
        let event: TouchpadEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event, TouchpadEvent::DragStart);
    }

    #[test]
    fn test_parse_drag_move() {
        let json = r#"{"action": "DRAGMOVE", "dx": 5.0, "dy": -1.5}"#;
        let event: TouchpadEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event, TouchpadEvent::DragMove { dx: 5.0, dy: -1.5 });
    }

    #[test]
    fn test_parse_drag_end() {
        let json = r#"{"action": "DRAGEND"}"#;
        let event: TouchpadEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event, TouchpadEvent::DragEnd);
    }

    #[test]
    fn test_parse_scroll_new() {
        let json = r#"{"action": "SCROLL", "dx": -1.0, "dy": 2.5, "ctrl": true}"#;
        let event: TouchpadEvent = serde_json::from_str(json).unwrap();
        assert_eq!(
            event,
            TouchpadEvent::Scroll {
                dx: -1.0,
                dy: 2.5,
                ctrl: true
            }
        );
    }

    #[test]
    fn test_parse_swipe() {
        let json = r#"{"action": "SWIPE", "direction": "UP"}"#;
        let event: TouchpadEvent = serde_json::from_str(json).unwrap();
        assert_eq!(
            event,
            TouchpadEvent::Swipe {
                direction: "UP".to_string()
            }
        );
    }

    #[test]
    fn test_parse_configure() {
        let json = r#"{"action": "CONFIGURE", "cursor_speed": 1.5, "scroll_speed": 2.0}"#;
        let event: TouchpadEvent = serde_json::from_str(json).unwrap();
        assert_eq!(
            event,
            TouchpadEvent::Configure {
                cursor_speed: 1.5,
                scroll_speed: 2.0
            }
        );
    }
}
