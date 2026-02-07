// Diction Desktop — Frontend
// Audio capture via AudioWorklet, IPC to Rust backend via Tauri

const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const startBtn = document.getElementById("startBtn");
const stopBtn = document.getElementById("stopBtn");
const settingsBtn = document.getElementById("settingsBtn");
const settingsPanel = document.getElementById("settingsPanel");
const saveSettings = document.getElementById("saveSettings");
const statusDot = document.getElementById("statusDot");
const statusText = document.getElementById("statusText");
const transcriptText = document.getElementById("transcriptText");
const partialText = document.getElementById("partialText");
const micSelect = document.getElementById("micSelect");

let audioContext = null;
let workletNode = null;
let mediaStream = null;
let isRecording = false;
let lastVoiceTs = 0;
let isSending = false;
let preroll = [];
let prerollMs = 0;

// Serial send queue — guarantees chunk ordering across async IPC calls.
const sendQueue = [];
let draining = false;

// Gate stats — logged periodically to show send vs suppress ratio.
let gateSent = 0;
let gateSuppressed = 0;

const SAMPLE_RATE = 24000;
const THRESHOLD_16 = 150;
const THRESHOLD_FLOAT = THRESHOLD_16 / 32768;
const HANGOVER_MS = 300;
const PREROLL_MS = 100;

// --- Status ---

function setStatus(text, state) {
  statusText.textContent = text;
  statusDot.className = `dot ${state}`;
}

// --- Audio ---

function floatTo16BitPCM(float32Array) {
  const buffer = new ArrayBuffer(float32Array.length * 2);
  const view = new DataView(buffer);
  for (let i = 0; i < float32Array.length; i++) {
    let sample = Math.max(-1, Math.min(1, float32Array[i]));
    sample = sample < 0 ? sample * 0x8000 : sample * 0x7fff;
    view.setInt16(i * 2, sample, true);
  }
  return new Uint8Array(buffer);
}

// Enqueue audio (Uint8Array) or "commit" (null) and drain serially.
function enqueueSend(pcm16OrNull) {
  sendQueue.push(pcm16OrNull);
  drainSendQueue();
}

async function drainSendQueue() {
  if (draining) return;
  draining = true;
  while (sendQueue.length > 0) {
    const item = sendQueue.shift();
    try {
      if (item === null) {
        await invoke("commit_audio");
      } else {
        await invoke("send_audio", { data: Array.from(item) });
      }
    } catch (err) {
      console.error("send/commit error:", err);
    }
  }
  draining = false;
}

async function startRecording() {
  if (isRecording) return;
  isRecording = true;
  startBtn.disabled = true;
  stopBtn.disabled = false;

  try {
    const constraints = { audio: true };
    const deviceId = micSelect.value;
    if (deviceId) {
      constraints.audio = { deviceId: { exact: deviceId } };
    }

    mediaStream = await navigator.mediaDevices.getUserMedia(constraints);
    audioContext = new AudioContext({ sampleRate: SAMPLE_RATE });
    await audioContext.audioWorklet.addModule("audio-worklet.js");

    const source = audioContext.createMediaStreamSource(mediaStream);
    workletNode = new AudioWorkletNode(audioContext, "pcm-processor");
    source.connect(workletNode).connect(audioContext.destination);

    workletNode.port.onmessage = (event) => {
      if (!isRecording) return;
      const float32 = event.data;
      let peak = 0;
      for (let i = 0; i < float32.length; i++) {
        const abs = Math.abs(float32[i]);
        if (abs > peak) peak = abs;
      }
      const now = performance.now();
      const hasVoice = peak >= THRESHOLD_FLOAT;
      if (hasVoice) lastVoiceTs = now;
      const inHangover = now - lastVoiceTs <= HANGOVER_MS;

      const pcm16 = floatTo16BitPCM(float32);
      const chunkMs = (float32.length / SAMPLE_RATE) * 1000;

      // Maintain preroll buffer while silent
      preroll.push(pcm16);
      prerollMs += chunkMs;
      while (prerollMs > PREROLL_MS) {
        const dropped = preroll.shift();
        if (!dropped) break;
        const droppedMs = (dropped.length / 2 / SAMPLE_RATE) * 1000;
        prerollMs -= droppedMs;
      }

      if (!hasVoice && !inHangover) {
        gateSuppressed++;
        if (isSending) enqueueSend(null); // commit
        isSending = false;
        // Log gate stats every ~5s (50 chunks × 100ms)
        if ((gateSent + gateSuppressed) % 50 === 0) {
          const total = gateSent + gateSuppressed;
          const pct = total > 0 ? ((gateSuppressed / total) * 100).toFixed(0) : 0;
          console.log(`[gate] sent=${gateSent} suppressed=${gateSuppressed} (${pct}% saved)`);
        }
        return;
      }

      gateSent++;
      if (hasVoice && !isSending) {
        // Preroll already contains the current chunk; flush it all.
        isSending = true;
        for (const buf of preroll) {
          enqueueSend(buf);
        }
        preroll = [];
        prerollMs = 0;
      } else {
        enqueueSend(pcm16);
      }
      isSending = true;
    };

    await invoke("start_session");
    setStatus("Listening", "live");
  } catch (err) {
    console.error("startRecording error:", err);
    const msg = typeof err === "string" ? err : "Failed to start";
    setStatus(msg, "error");
    // Ensure mic and audio context are released on failure
    if (workletNode) {
      workletNode.disconnect();
      workletNode = null;
    }
    if (mediaStream) {
      mediaStream.getTracks().forEach((t) => t.stop());
      mediaStream = null;
    }
    if (audioContext) {
      await audioContext.close();
      audioContext = null;
    }
    isRecording = false;
    startBtn.disabled = false;
    stopBtn.disabled = true;
  }
}

async function stopRecording() {
  if (!isRecording) return;
  isRecording = false;
  startBtn.disabled = false;
  stopBtn.disabled = true;

  try {
    await invoke("stop_session");
  } catch (err) {
    console.error("stop_session error:", err);
  }

  if (workletNode) {
    workletNode.disconnect();
    workletNode = null;
  }
  if (mediaStream) {
    mediaStream.getTracks().forEach((t) => t.stop());
    mediaStream = null;
  }
  if (audioContext) {
    await audioContext.close();
    audioContext = null;
  }
  lastVoiceTs = 0;
  isSending = false;
  preroll = [];
  prerollMs = 0;
  sendQueue.length = 0;
  draining = false;
  if (gateSent + gateSuppressed > 0) {
    const total = gateSent + gateSuppressed;
    const pct = ((gateSuppressed / total) * 100).toFixed(0);
    console.log(`[gate] session total: sent=${gateSent} suppressed=${gateSuppressed} (${pct}% saved)`);
  }
  gateSent = 0;
  gateSuppressed = 0;

  setStatus("Ready", "idle");
}

// --- Settings ---

const apiKeyInput = document.getElementById("apiKey");
const modelInput = document.getElementById("model");
const languageInput = document.getElementById("language");

settingsBtn.addEventListener("click", () => {
  settingsPanel.classList.toggle("hidden");
});

saveSettings.addEventListener("click", async () => {
  try {
    await invoke("set_setting", { key: "api_key", value: apiKeyInput.value });
    await invoke("set_setting", { key: "model", value: modelInput.value });
    await invoke("set_setting", { key: "language", value: languageInput.value });
    if (micSelect.value) {
      await invoke("set_setting", { key: "mic_device_id", value: micSelect.value });
    }
    setStatus("Settings saved", "idle");
  } catch (err) {
    console.error("saveSettings error:", err);
    setStatus("Save failed", "error");
  }
});

async function loadSettings() {
  try {
    const apiKey = await invoke("get_setting", { key: "api_key" });
    if (apiKey && apiKey !== null) apiKeyInput.value = apiKey;
    const model = await invoke("get_setting", { key: "model" });
    if (model && model !== null) modelInput.value = model;
    const language = await invoke("get_setting", { key: "language" });
    if (language && language !== null) languageInput.value = language;
    const micId = await invoke("get_setting", { key: "mic_device_id" });
    if (micId && micId !== null) micSelect.value = micId;
  } catch (err) {
    console.error("loadSettings error:", err);
  }
}

async function loadMicList() {
  try {
    const devices = await navigator.mediaDevices.enumerateDevices();
    const mics = devices.filter((d) => d.kind === "audioinput");
    micSelect.innerHTML = "";
    for (const mic of mics) {
      const opt = document.createElement("option");
      opt.value = mic.deviceId;
      opt.textContent = mic.label || `Microphone ${micSelect.options.length + 1}`;
      micSelect.appendChild(opt);
    }
  } catch (err) {
    console.error("enumerateDevices error:", err);
  }
}

// --- Events from Rust ---

listen("transcript", (event) => {
  const { text, is_final } = event.payload;
  if (is_final) {
    const spacer =
      transcriptText.textContent.endsWith("\n") ||
      transcriptText.textContent === ""
        ? ""
        : " ";
    transcriptText.textContent += spacer + text;
    partialText.textContent = "";
  } else {
    partialText.textContent = text;
  }
});

listen("status-update", (event) => {
  const { status, message } = event.payload;
  setStatus(message, status);
});

listen("hotkey-push", async () => {
  await startRecording();
});

listen("hotkey-release", async () => {
  await stopRecording();
});

// --- Init ---

startBtn.addEventListener("click", startRecording);
stopBtn.addEventListener("click", stopRecording);
loadMicList().then(loadSettings);
setStatus("Ready", "idle");
