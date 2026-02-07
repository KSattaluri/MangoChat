const { invoke } = window.__TAURI__.core;
const { listen, emit } = window.__TAURI__.event;

const canvas = document.getElementById("canvas");
const ctx = canvas.getContext("2d");

const img = new Image();
let imgReady = false;
let dragging = false;
let start = null;
let current = null;

const MIN_SIZE = 5;

function getScale() {
  const rect = canvas.getBoundingClientRect();
  return {
    rect,
    scaleX: canvas.width / rect.width,
    scaleY: canvas.height / rect.height,
  };
}

function toCanvasPoint(evt) {
  const { rect, scaleX, scaleY } = getScale();
  const x = (evt.clientX - rect.left) * scaleX;
  const y = (evt.clientY - rect.top) * scaleY;
  return { x, y };
}

function normalizeRect(a, b) {
  const x = Math.min(a.x, b.x);
  const y = Math.min(a.y, b.y);
  const w = Math.abs(a.x - b.x);
  const h = Math.abs(a.y - b.y);
  return { x, y, w, h };
}

function clampRect(rect) {
  const x = Math.max(0, Math.min(rect.x, canvas.width - 1));
  const y = Math.max(0, Math.min(rect.y, canvas.height - 1));
  const w = Math.min(rect.w, canvas.width - x);
  const h = Math.min(rect.h, canvas.height - y);
  return { x, y, w, h };
}

function drawSelection(rect) {
  if (!rect) return;
  const { x, y, w, h } = clampRect(rect);
  if (w <= 0 || h <= 0) return;

  ctx.drawImage(img, x, y, w, h, x, y, w, h);
  ctx.save();
  ctx.strokeStyle = "rgba(255,255,255,0.9)";
  ctx.lineWidth = 1;
  ctx.setLineDash([6, 4]);
  ctx.strokeRect(x + 0.5, y + 0.5, w, h);
  ctx.setLineDash([]);

  const label = `${Math.round(w)}×${Math.round(h)}`;
  ctx.font = "13px Segoe UI, system-ui, sans-serif";
  const padding = 6;
  const textWidth = ctx.measureText(label).width;
  const boxW = textWidth + padding * 2;
  const boxH = 20;
  const boxX = x + 8;
  const boxY = Math.max(8, y - 28);
  ctx.fillStyle = "rgba(0,0,0,0.6)";
  ctx.fillRect(boxX, boxY, boxW, boxH);
  ctx.fillStyle = "#fff";
  ctx.fillText(label, boxX + padding, boxY + 14);
  ctx.restore();
}

function draw() {
  if (!imgReady) return;
  ctx.clearRect(0, 0, canvas.width, canvas.height);
  ctx.drawImage(img, 0, 0, canvas.width, canvas.height);
  ctx.fillStyle = "rgba(0,0,0,0.4)";
  ctx.fillRect(0, 0, canvas.width, canvas.height);

  if (start && (dragging || current)) {
    const rect = normalizeRect(start, current || start);
    drawSelection(rect);
  }
}

canvas.addEventListener("pointerdown", (evt) => {
  if (!imgReady) return;
  dragging = true;
  start = toCanvasPoint(evt);
  current = start;
  canvas.setPointerCapture(evt.pointerId);
  draw();
});

canvas.addEventListener("pointermove", (evt) => {
  if (!dragging) return;
  current = toCanvasPoint(evt);
  draw();
});

canvas.addEventListener("pointerup", async (evt) => {
  if (!dragging) return;
  dragging = false;
  current = toCanvasPoint(evt);
  const rect = normalizeRect(start, current);
  start = null;
  current = null;
  draw();

  const { x, y, w, h } = clampRect(rect);
  if (w < MIN_SIZE || h < MIN_SIZE) {
    await invoke("cancel_snip");
    return;
  }

  await invoke("finish_snip", {
    x: Math.round(x),
    y: Math.round(y),
    width: Math.round(w),
    height: Math.round(h),
  });
});

window.addEventListener("keydown", async (evt) => {
  if (evt.key === "Escape") {
    await invoke("cancel_snip");
  }
});

listen("snip-screenshot", (event) => {
  const b64 = event.payload;
  // Reset selection state for new snip
  imgReady = false;
  dragging = false;
  start = null;
  current = null;

  img.onload = () => {
    canvas.width = img.width;
    canvas.height = img.height;
    imgReady = true;
    draw();
  };
  img.src = `data:image/jpeg;base64,${b64}`;
});

// Signal to Rust that the overlay JS is loaded and ready for screenshot data.
emit("snip-ready");
