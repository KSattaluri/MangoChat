# Mango Chat FAQ

## What happens when you start Mango Chat?
When you start recording, Mango Chat listens for audio from your device and sends speech to the currently selected transcription source. In cloud mode, that means your selected provider. In local mode, that means the bundled Whisper.cpp path. Place your cursor in a text field to begin dictating.

## How do I quit Mango Chat?
Open the system tray and click Quit.

## Why do I need API keys?
API keys are required only for cloud providers. Local Whisper mode does not use a provider API key. You can sign up for Deepgram and AssemblyAI to get up to $250 in trial credits with no credit card.

## Where are my API keys stored?
API keys are encrypted at rest and stored locally on your machine in `AppData/Local/MangoChat`. They are only transmitted over secure connections when authenticating with your chosen provider.

## Does Mango Chat collect telemetry or personal information?
Mango Chat has no built-in telemetry. In cloud mode, audio is sent only to your selected provider for transcription. In local Whisper mode, transcription stays on-device.

## What are the hotkeys to start and stop Mango Chat?
In addition to the start/stop buttons on the UI, you can use `Right Ctrl` to start and stop recording when that hotkey is enabled in settings.

## Why do I sometimes experience delays or inaccurate transcription?
These vary by transcription mode and provider. Cloud behavior depends on network quality and provider characteristics. Local Whisper behavior depends on your machine, the bundled model, and current VAD segmentation.

## How do I take a screenshot?
When screenshot capture is enabled, move your cursor to the monitor you want, press `Right Alt`, then select the region.

## What happens after I capture a screenshot?
Based on your settings, Mango Chat can copy the image path, copy the image content, or open it in Paint for editing.

## Where are screenshots saved?
Use `Open images folder` in Settings to open the active screenshot directory.

## How much does transcription cost?
It depends on the chosen provider and model. Pricing is typically per second or per hour. Deepgram and AssemblyAI often provide free trial credits; check their sites for current details.

## Which providers are supported?
Cloud providers: Deepgram, OpenAI Realtime, ElevenLabs Realtime, and AssemblyAI.

Local transcription option: Whisper.cpp.

## Do I need the internet to dictate?
Cloud mode requires network access. Local Whisper mode does not require provider connectivity for transcription.

## Can I customize commands and aliases?
Yes. You can edit browser commands, text aliases, and app locations from the Commands tab.
