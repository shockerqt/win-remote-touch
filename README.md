# win-remote-touch

Aplicación cliente-servidor para usar un dispositivo táctil (como un móvil o tablet) como pantalla táctil o touchpad remoto para un entorno Windows.

- **Servidor:** Rust (usando `windows-sys` para `InjectTouchInput` y WebSockets para la red).
- **Cliente:** Interfaz web para capturar toques desde cualquier móvil.
