const toggleBtn = document.getElementById("toggleBtn");
const copyBtn = document.getElementById("copyBtn");
const reviseBtn = document.getElementById("reviseBtn");
const clearBtn = document.getElementById("clearBtn");
const tabRaw = document.getElementById("tabRaw");
const tabRevised = document.getElementById("tabRevised");
const statusText = document.getElementById("statusText");
const statusDot = document.getElementById("statusDot");
const finalText = document.getElementById("finalText");
const revisedText = document.getElementById("revisedText");
const partialText = document.getElementById("partialText");

let audioContext = null;
let workletNode = null;
let mediaStream = null;
let websocket = null;
let isRecording = false;
let editHistory = [];
let finalSegments = [];
let commandActive = false;
let commandParts = [];
let commandBlocks = [];
const NEWLINE_COMMANDS = new Set([
  "enter",
  "new line",
  "newline",
  "line break",
]);
const NEW_PARAGRAPH_COMMANDS = new Set([
  "new paragraph",
  "new para",
  "paragraph",
]);

function setStatus(text, live) {
  statusText.textContent = text;
  statusDot.className = `dot ${live ? "live" : "idle"}`;
}

function appendFinal(text) {
  if (!text) return;
  const normalized = text
    .trim()
    .toLowerCase()
    .replace(/[.?!]+$/g, "");
  if (normalized === "command") {
    commandActive = true;
  } else if (normalized === "end command") {
    commandActive = false;
    if (commandParts.length) {
      commandBlocks.push(commandParts.join(" "));
      commandParts = [];
    }
  } else if (commandActive) {
    commandParts.push(text);
  }
  if (NEW_PARAGRAPH_COMMANDS.has(normalized)) {
    appendParagraphBreak();
    return;
  }
  if (NEWLINE_COMMANDS.has(normalized)) {
    finalText.textContent += "\n";
    return;
  }
  const spacer = finalText.textContent.endsWith("\n") || finalText.textContent === "" ? "" : " ";
  finalText.textContent += spacer + text;
  finalSegments.push({ text, spacer });
}

function appendParagraphBreak() {
  if (!finalText.textContent.endsWith("\n\n")) {
    finalText.textContent += "\n\n";
  }
}

function clearPartial() {
  partialText.textContent = "";
}

function setActiveTab(tab) {
  if (tab === "raw") {
    tabRaw.classList.add("active");
    tabRevised.classList.remove("active");
    finalText.classList.remove("hidden");
    revisedText.classList.add("hidden");
    return;
  }
  tabRevised.classList.add("active");
  tabRaw.classList.remove("active");
  revisedText.classList.remove("hidden");
  finalText.classList.add("hidden");
}

function applyEditOp(op) {
  if (!op || !op.op) return;
  if (op.op === "undo") {
    const previous = editHistory.pop();
    if (previous !== undefined) {
      finalText.textContent = previous;
    }
    return;
  }
  if (op.op === "none") return;

  const original = finalText.textContent;
  editHistory.push(original);

  const scope = op.scope || "full";
  const occurrence = op.occurrence || "first";
  const target = (op.target || "").trim();

  const applyWithin = (text, replacer) => {
    if (scope === "last_paragraph") {
      const parts = text.split("\n\n");
      const last = parts.pop() || "";
      const updated = replacer(last);
      return [...parts, updated].join("\n\n");
    }
    if (scope === "last_sentence") {
      const idx = Math.max(
        text.lastIndexOf("."),
        text.lastIndexOf("!"),
        text.lastIndexOf("?")
      );
      if (idx === -1) return replacer(text);
      const head = text.slice(0, idx + 1);
      const tail = text.slice(idx + 1);
      return head + replacer(tail);
    }
    return replacer(text);
  };

  const replaceOnce = (text, from, to, which) => {
    if (!from) return text;
    if (which === "all") return text.split(from).join(to);
    if (which === "last") {
      const idx = text.lastIndexOf(from);
      if (idx === -1) return text;
      return text.slice(0, idx) + to + text.slice(idx + from.length);
    }
    const idx = text.indexOf(from);
    if (idx === -1) return text;
    return text.slice(0, idx) + to + text.slice(idx + from.length);
  };

  const insertRelative = (text, from, insertion, which, before) => {
    if (!from) return text;
    const idx = which === "last" ? text.lastIndexOf(from) : text.indexOf(from);
    if (idx === -1) return text;
    const insertAt = before ? idx : idx + from.length;
    return text.slice(0, insertAt) + insertion + text.slice(insertAt);
  };

  const updated = applyWithin(original, (chunk) => {
    if (op.op === "replace") {
      return replaceOnce(chunk, target, op.with || "", occurrence);
    }
    if (op.op === "delete") {
      return replaceOnce(chunk, target, "", occurrence);
    }
    if (op.op === "insert_before") {
      return insertRelative(chunk, target, op.text || "", occurrence, true);
    }
    if (op.op === "insert_after") {
      return insertRelative(chunk, target, op.text || "", occurrence, false);
    }
    return chunk;
  });

  finalText.textContent = updated;
}

function floatTo16BitPCM(float32Array) {
  const buffer = new ArrayBuffer(float32Array.length * 2);
  const view = new DataView(buffer);
  for (let i = 0; i < float32Array.length; i += 1) {
    let sample = Math.max(-1, Math.min(1, float32Array[i]));
    sample = sample < 0 ? sample * 0x8000 : sample * 0x7fff;
    view.setInt16(i * 2, sample, true);
  }
  return buffer;
}

async function startRecording() {
  if (isRecording) return;
  isRecording = true;
  toggleBtn.textContent = "Stop";
  toggleBtn.classList.add("stop");
  setStatus("Listening", true);

  websocket = new WebSocket(`ws://${location.host}/ws`);
  websocket.onmessage = (event) => {
    const payload = JSON.parse(event.data);
    if (payload.type === "error") {
      setStatus(payload.message, false);
      return;
    }
    if (payload.type === "clarify") {
      setStatus(payload.message, true);
      partialText.textContent = "";
      return;
    }
    if (payload.type === "retract_last") {
      const last = finalSegments.pop();
      if (last) {
        const suffix = `${last.spacer}${last.text}`;
        if (finalText.textContent.endsWith(suffix)) {
          finalText.textContent = finalText.textContent.slice(0, -suffix.length);
        }
      }
      return;
    }
    if (payload.type === "edit_op") {
      applyEditOp(payload.op || {});
      setStatus("Edit applied", true);
      clearPartial();
      return;
    }
    if (payload.type === "transcript") {
      if (payload.is_final) {
        appendFinal(payload.text);
        clearPartial();
      } else {
        partialText.textContent = payload.text;
      }
    }
  };

  mediaStream = await navigator.mediaDevices.getUserMedia({ audio: true });
  audioContext = new AudioContext({ sampleRate: 24000 });
  await audioContext.audioWorklet.addModule("/static/audio-worklet.js");

  const source = audioContext.createMediaStreamSource(mediaStream);
  workletNode = new AudioWorkletNode(audioContext, "pcm-processor");
  source.connect(workletNode).connect(audioContext.destination);

  workletNode.port.onmessage = (event) => {
    if (!websocket || websocket.readyState !== WebSocket.OPEN) {
      return;
    }
    const pcm16 = floatTo16BitPCM(event.data);
    websocket.send(pcm16);
  };
}

async function stopRecording() {
  if (!isRecording) return;
  isRecording = false;
  toggleBtn.textContent = "Start";
  toggleBtn.classList.remove("stop");
  setStatus("Ready", false);

  if (websocket && websocket.readyState === WebSocket.OPEN) {
    websocket.send("stop");
    websocket.close();
  }
  websocket = null;

  if (workletNode) {
    workletNode.disconnect();
    workletNode = null;
  }

  if (mediaStream) {
    mediaStream.getTracks().forEach((track) => track.stop());
    mediaStream = null;
  }

  if (audioContext) {
    await audioContext.close();
    audioContext = null;
  }
}

toggleBtn.addEventListener("click", async () => {
  if (isRecording) {
    await stopRecording();
  } else {
    await startRecording();
  }
});

copyBtn.addEventListener("click", async () => {
  const text = finalText.textContent.trim();
  if (!text) return;
  await navigator.clipboard.writeText(text);
  copyBtn.textContent = "Copied";
  setTimeout(() => {
    copyBtn.textContent = "Copy";
  }, 1200);
});

if (clearBtn) {
  clearBtn.addEventListener("click", () => {
    finalText.textContent = "";
    revisedText.textContent = "";
    partialText.textContent = "";
    finalSegments = [];
    editHistory = [];
    commandActive = false;
    commandParts = [];
    commandBlocks = [];
    setActiveTab("raw");
    setStatus("Ready", false);
  });
}

reviseBtn.addEventListener("click", async () => {
  const raw = finalText.textContent.trim();
  if (!raw) return;
  if (commandActive && commandParts.length) {
    commandBlocks.push(commandParts.join(" "));
    commandParts = [];
  }
  reviseBtn.disabled = true;
  setStatus("Revising...", true);
  revisedText.textContent = "";
  try {
    const start = performance.now();
    const response = await fetch("/revise", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ raw_text: raw, commands: commandBlocks }),
    });
    const data = await response.json();
    const totalMs = Math.round(performance.now() - start);
    if (data.error) {
      setStatus(data.error, false);
      reviseBtn.disabled = false;
      return;
    }
    revisedText.textContent = data.revised_text || "";
    setActiveTab("revised");
    setStatus(`Revision ready (${totalMs}ms)`, true);
  } catch (err) {
    setStatus("Revision failed", false);
  } finally {
    reviseBtn.disabled = false;
  }
});

tabRaw.addEventListener("click", () => setActiveTab("raw"));
tabRevised.addEventListener("click", () => setActiveTab("revised"));

setStatus("Ready", false);
