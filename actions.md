1. Confirm product scope and success criteria for the Windows-native Tauri app.
2. Decide the Windows input injection approach and tray/hotkey behavior.
3. Define the first vertical slice (audio capture → STT → text injection) and its UX.
4. Scaffold the Tauri app structure and wire the WebView UI.
5. Port the POC WebAudio capture into the Tauri WebView.
6. Implement the WebSocket client (JS first), connect to OpenAI Realtime, and stream audio.
7. Implement transcript handling and text/keystroke injection into the focused app.
8. Add basic settings UI for API keys and model selection.
9. Add persistence for raw/revised transcripts and session history.
10. Harden reliability (reconnects, background behavior, error handling) and package Windows build.
