// Diction Desktop — Frontend
// Audio capture via AudioWorklet, IPC to Rust backend via Tauri

const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;
const { getCurrentWindow } = window.__TAURI__.window;
const { LogicalSize } = window.__TAURI__.dpi;

const appWindow = getCurrentWindow();

const startBtn = document.getElementById("startBtn");
const stopBtn = document.getElementById("stopBtn");
const settingsBtn = document.getElementById("settingsBtn");
const openFolderBtn = document.getElementById("openFolderBtn");
const closeBtn = document.getElementById("closeBtn");
const settingsPanel = document.getElementById("settingsPanel");
const saveSettings = document.getElementById("saveSettings");
const statusDot = document.getElementById("statusDot");
const statusText = document.getElementById("statusText");
const visualizerCanvas = document.getElementById("visualizer");
const micSelect = document.getElementById("micSelect");

const WIN_W = 260;
const WIN_H_COMPACT = 72;
const WIN_H_SETTINGS = 330;

let audioContext = null;
let workletNode = null;
let analyserNode = null;
let visualizerAnimId = null;
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

let errorTimeout = null;

function setStatus(text, state) {
  statusText.textContent = text;
  statusDot.className = `dot ${state}`;
  // Show text, hide visualizer for non-live states
  if (state !== "live") {
    stopVisualizer();
  }
  // Auto-recover from error after 4 seconds
  if (errorTimeout) {
    clearTimeout(errorTimeout);
    errorTimeout = null;
  }
  if (state === "error") {
    errorTimeout = setTimeout(() => {
      setStatus("Ready", "idle");
    }, 4000);
  }
}

// --- Visualizer ---

function startVisualizer(analyser) {
  const ctx = visualizerCanvas.getContext("2d");
  const bufferLength = analyser.frequencyBinCount;
  const dataArray = new Uint8Array(bufferLength);
  const barCount = 8;
  const gap = 2;
  const barWidth = (visualizerCanvas.width - gap * (barCount - 1)) / barCount;

  statusText.classList.add("hidden");
  visualizerCanvas.classList.remove("hidden");

  function draw() {
    if (!isRecording) {
      ctx.clearRect(0, 0, visualizerCanvas.width, visualizerCanvas.height);
      return;
    }
    visualizerAnimId = requestAnimationFrame(draw);

    analyser.getByteFrequencyData(dataArray);
    ctx.clearRect(0, 0, visualizerCanvas.width, visualizerCanvas.height);

    for (let i = 0; i < barCount; i++) {
      // Sample from lower frequencies (more visually active)
      const idx = Math.floor((i * bufferLength * 0.5) / barCount);
      const value = dataArray[idx] / 255;
      const barHeight = Math.max(2, value * visualizerCanvas.height);
      const x = i * (barWidth + gap);
      const y = visualizerCanvas.height - barHeight;

      ctx.fillStyle = "#36d399";
      ctx.shadowColor = "#36d399";
      ctx.shadowBlur = 4;
      ctx.fillRect(x, y, barWidth, barHeight);
    }
    ctx.shadowBlur = 0;
  }

  draw();
}

function stopVisualizer() {
  if (visualizerAnimId) {
    cancelAnimationFrame(visualizerAnimId);
    visualizerAnimId = null;
  }
  visualizerCanvas.classList.add("hidden");
  statusText.classList.remove("hidden");
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

    // Branch analyser off source for visualizer (doesn't affect audio chain)
    analyserNode = audioContext.createAnalyser();
    analyserNode.fftSize = 64;
    analyserNode.smoothingTimeConstant = 0.8;
    source.connect(analyserNode);

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
        // Log gate stats every ~5s (50 chunks * 100ms)
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
    analyserNode = null;
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

  stopVisualizer();
  analyserNode = null;
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

settingsBtn.addEventListener("click", async () => {
  settingsPanel.classList.toggle("hidden");
  const isOpen = !settingsPanel.classList.contains("hidden");
  const newH = isOpen ? WIN_H_SETTINGS : WIN_H_COMPACT;
  const delta = WIN_H_SETTINGS - WIN_H_COMPACT;
  // Expand upward: shift Y so the bottom edge stays anchored
  const pos = await appWindow.outerPosition();
  const scale = await appWindow.scaleFactor();
  const logicalY = pos.y / scale;
  const newY = isOpen ? logicalY - delta : logicalY + delta;
  await appWindow.setPosition(
    new window.__TAURI__.dpi.LogicalPosition(pos.x / scale, newY),
  );
  await appWindow.setSize(new LogicalSize(WIN_W, newH));
});

saveSettings.addEventListener("click", async () => {
  try {
    await invoke("set_setting", { key: "api_key", value: apiKeyInput.value });
    await invoke("set_setting", { key: "model", value: modelInput.value });
    await invoke("set_setting", { key: "language", value: languageInput.value });
    if (micSelect.value) {
      await invoke("set_setting", { key: "mic_device_id", value: micSelect.value });
    }
    setStatus("Saved", "idle");
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

// --- Open snip folder ---

openFolderBtn.addEventListener("click", async () => {
  try {
    await invoke("open_snip_folder");
  } catch (err) {
    console.error("open_snip_folder error:", err);
  }
});

// --- Events from Rust ---

listen("transcript", (event) => {
  const { text } = event.payload;
  console.log("[transcript]", text);
});

listen("status-update", (event) => {
  const { status, message } = event.payload;
  setStatus(message, status);
  // "Listening" from Rust means WebSocket is connected and session is configured.
  // That's when we show the visualizer — not before.
  if (status === "live" && message === "Listening" && analyserNode && isRecording) {
    startVisualizer(analyserNode);
  }
});

listen("hotkey-push", async () => {
  await startRecording();
});

listen("hotkey-release", async () => {
  await stopRecording();
});

listen("snip-complete", (event) => {
  console.log("[snip] saved:", event.payload);
});

// --- Close to tray ---

closeBtn.addEventListener("click", () => {
  appWindow.hide();
});

// --- Init ---

startBtn.addEventListener("click", startRecording);
stopBtn.addEventListener("click", stopRecording);
loadMicList().then(loadSettings);
setStatus("Ready", "idle");
