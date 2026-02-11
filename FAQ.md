# Jarvis FAQ

## 1) How do I start dictating?
Press and hold `Right Ctrl` and speak. Release to commit your speech to the active text field.

## 2) What is the quick toggle behavior?
Tap `Right Ctrl` to quickly arm/disarm dictation.

## 3) Which speech providers are supported?
`OpenAI Realtime`, `Deepgram`, `ElevenLabs Realtime`, and `AssemblyAI`.

## 4) Why is my speech not transcribing?
Check these first:
- A provider API key is entered and validated in `Settings > Provider`.
- A default provider is selected.
- The microphone device in `Settings > Audio` is correct.
- The app is armed.

## 5) Why do I have to click Save in Provider settings?
Provider changes are staged in the form first. They are only persisted/applied after `Save`.

## 6) What do Validate and Default mean in Provider settings?
- `Validate`: checks whether the current API key works for that provider.
- `Default`: selects the provider Jarvis uses for live dictation.

## 7) Can I use different API keys for each provider?
Yes. Keys are stored per provider.

## 8) Where are my settings saved?
On Windows, settings are stored in:
`AppData/Local/Jarvis/settings.json`

## 9) Can I change microphones without restarting the app?
Yes. In `Settings > Audio`, use the microphone dropdown and click `Refresh` to re-scan available input devices.

## 10) What is VAD mode, and which one should I use?
- `Strict`: tighter filtering, better for noisy environments.
- `Lenient`: more sensitive pickup, better for quiet speech.
If words are getting missed, try `Lenient`. If noise triggers speech, use `Strict`.

## 11) What is the Start Cue option?
Start Cue plays a short audio notification when capture starts, so you know the pipeline is active.

## 12) How do voice URL commands work?
In `Settings > Commands`, map a trigger word (for example `github`) to a URL. Saying `open <trigger>` opens that URL in Chrome.

## 13) Can Jarvis run edit commands like Enter/Backspace?
Yes. Jarvis supports built-in voice command keywords (for example `enter`, `back`) and dispatches them as keyboard actions.

## 14) Why do some very short words sometimes not appear?
Short utterances can be affected by VAD segmentation and provider turn-finalization behavior. If this happens often, try `Lenient` mode and speak command words clearly with a brief pause after them.

## 15) What is the Usage tab for?
It shows session and cumulative usage metrics (for example sent/suppressed audio time, bytes sent, and commits), plus recent session history.

## 16) What happens when I close the app window?
Closing the main window hides Jarvis to tray by default (unless explicitly quitting).

## 17) How do I fully quit Jarvis?
Use tray menu `Quit` (or the app's explicit quit control where available).

## 18) How do screenshots/snips work?
Use the snip workflow to select an area. You can copy image data, copy file path, and optionally open/edit after capture (for example in Paint).

## 19) Why does provider behavior feel different across services?
Each provider has different server-side turn detection and finalization behavior. Jarvis uses one client pipeline, but transcript timing and punctuation can vary by provider.

## 20) Is there a light theme?
No. The app currently uses a dark theme only.

## 21) Can I change the hotkey from Right Ctrl?
Not currently in settings. Right Ctrl is the active hotkey.

## 22) What should I do if tray actions or transcription look stuck?
Try this sequence:
- Disarm and re-arm.
- Confirm provider key/default in Provider settings.
- Confirm microphone selection in Audio settings.
- Restart the app if needed.
