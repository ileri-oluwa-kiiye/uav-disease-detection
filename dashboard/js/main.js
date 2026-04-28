const API_BASE = "http://127.0.0.1:8000";
const TABS = ["dashboard", "map", "history"];

const TOPBAR_COPY = {
  dashboard: {
    title: "Mission dashboard",
    subtitle:
      "Control the UAV, monitor telemetry, upload field imagery, and review detections.",
  },
  map: {
    title: "Map view",
    subtitle: "Clustered markers from GET /predictions — click for full context.",
  },
  history: {
    title: "Flight history",
    subtitle: "Sort and filter past runs for reporting and traceability.",
  },
};

const DROP_ACTIVE = "drop-zone--active";

let historyRows = [];
let mapInstance = null;
let clusterGroup = null;
let mapContainerId = "map-view-host";
let looseMarkers = [];

const controlState = {
  armed: false,
  throttle: 0,
  position: { x: 0, y: 0, z: 0 },
  orientation: { roll: 0, pitch: 0, yaw: 0 },
  tick: 0,
};

function getEl(id) {
  return document.getElementById(id);
}

function clamp(value, min, max) {
  return Math.min(max, Math.max(min, value));
}

function readNumber(id, fallback = 0) {
  const value = Number(getEl(id)?.value);
  return Number.isFinite(value) ? value : fallback;
}

async function handleResponse(response) {
  const text = await response.text();
  let data = null;
  try {
    data = text ? JSON.parse(text) : null;
  } catch {
    data = { detail: text || "Invalid response from server" };
  }
  if (!response.ok) {
    const msg =
      (data && (data.detail || data.message)) ||
      `Request failed (${response.status})`;
    const err = new Error(typeof msg === "string" ? msg : JSON.stringify(msg));
    err.status = response.status;
    err.data = data;
    throw err;
  }
  return data;
}

async function postPredict(formData) {
  const res = await fetch(`${API_BASE}/predict`, {
    method: "POST",
    body: formData,
  });
  return handleResponse(res);
}

async function fetchPredictions() {
  const res = await fetch(`${API_BASE}/predictions`);
  return handleResponse(res);
}

function isHealthyLabel(prediction) {
  if (!prediction) return false;
  return String(prediction).toLowerCase().includes("healthy");
}

function escapeHtml(s) {
  const div = document.createElement("div");
  div.textContent = s == null ? "" : String(s);
  return div.innerHTML;
}

function formatTs(iso) {
  try {
    const d = new Date(iso);
    return d.toLocaleString();
  } catch {
    return iso;
  }
}

function isDataImageUrl(u) {
  return (
    typeof u === "string" &&
    /^data:image\/(jpeg|jpg|png|webp);base64,/i.test(u)
  );
}

function setupDropZone(zone, fileInput, preview, onFile) {
  if (!zone || !fileInput || !preview) return;

  const activate = () => zone.classList.add(DROP_ACTIVE);
  const deactivate = () => zone.classList.remove(DROP_ACTIVE);

  zone.addEventListener("click", () => fileInput.click());
  zone.addEventListener("keydown", (e) => {
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      fileInput.click();
    }
  });

  zone.addEventListener("dragenter", (e) => {
    e.preventDefault();
    activate();
  });
  zone.addEventListener("dragover", (e) => {
    e.preventDefault();
    activate();
  });
  zone.addEventListener("dragleave", (e) => {
    e.preventDefault();
    if (!zone.contains(e.relatedTarget)) deactivate();
  });
  zone.addEventListener("drop", (e) => {
    e.preventDefault();
    deactivate();
    const f = e.dataTransfer?.files?.[0];
    if (f && f.type.startsWith("image/")) {
      fileInput.files = e.dataTransfer.files;
      showPreview(f, preview);
      onFile?.(f);
    }
  });

  fileInput.addEventListener("change", () => {
    const f = fileInput.files?.[0];
    if (f) {
      showPreview(f, preview);
      onFile?.(f);
    }
  });
}

function showPreview(file, preview) {
  const url = URL.createObjectURL(file);
  if (preview.dataset.objectUrl) URL.revokeObjectURL(preview.dataset.objectUrl);
  preview.dataset.objectUrl = url;
  preview.src = url;
  preview.hidden = false;
  preview.closest(".preview-wrap")?.classList.add("preview-wrap--visible");
}

function setSubmitLoading(btn, loading) {
  if (!btn) return;
  btn.disabled = loading;
  btn.classList.toggle("btn--loading", loading);
  const label = btn.querySelector(".btn-label");
  const spin = btn.querySelector(".btn-spinner");
  if (label) label.hidden = loading;
  if (spin) spin.hidden = !loading;
}

function renderPredictionCard(card, payload) {
  if (!card) return;
  card.hidden = false;
  const healthy = isHealthyLabel(payload.prediction);
  card.classList.toggle("result-card--healthy", healthy);
  card.classList.toggle("result-card--risk", !healthy);

  const title = card.querySelector("[data-field='class']");
  const confBar = card.querySelector("[data-field='conf-bar']");
  const confText = card.querySelector("[data-field='conf-text']");
  const coords = card.querySelector("[data-field='coords']");
  const img = card.querySelector("[data-field='result-img']");
  const timeEl = card.querySelector("[data-field='time']");

  if (title) title.textContent = payload.prediction || "—";
  const pct = Math.round(Number(payload.confidence) * 1000) / 10;
  if (confBar) confBar.style.width = `${Math.min(100, Math.max(0, pct))}%`;
  if (confText) confText.textContent = `${Number.isFinite(pct) ? pct : 0}%`;
  if (coords) {
    coords.textContent = `${Number(payload.latitude).toFixed(5)}, ${Number(payload.longitude).toFixed(5)}`;
  }
  if (timeEl) timeEl.textContent = payload.timestamp ? formatTs(payload.timestamp) : "—";

  if (img && payload.previewUrl) {
    img.src = payload.previewUrl;
    img.hidden = false;
  }
}

function hidePredictionCard(card) {
  if (card) card.hidden = true;
}

function showError(el, message) {
  if (!el) return;
  el.textContent = message || "";
  el.hidden = !message;
  el.classList.toggle("alert--visible", !!message);
}

function updateTopbar(name) {
  const copy = TOPBAR_COPY[name] || TOPBAR_COPY.dashboard;
  const titleEl = getEl("topbar-title");
  const subEl = document.querySelector(".topbar__subtitle");
  if (titleEl) titleEl.textContent = copy.title;
  if (subEl) subEl.textContent = copy.subtitle;
}

function setActiveTab(name) {
  updateTopbar(name);
  TABS.forEach((t) => {
    const panel = getEl(`panel-${t}`);
    const nav = document.querySelector(`[data-nav="${t}"]`);
    if (panel) panel.hidden = t !== name;
    if (nav) nav.classList.toggle("nav-item--active", t === name);
  });

  if (name === "map") {
    showError(getEl("map-error"), "");
    initMap("map-view-host");
    invalidateMapSize();
    refreshMapMarkers().catch((e) =>
      showError(getEl("map-error"), e.message || "Could not load predictions.")
    );
  }

  if (name === "history") {
    loadHistoryTable();
  }
}

function markerColor(prediction) {
  return isHealthyLabel(prediction) ? "#22c55e" : "#ef4444";
}

function buildPopupHtml(p) {
  const conf = (Number(p.confidence) * 100).toFixed(1);
  const img = isDataImageUrl(p.image_url)
    ? `<img class="map-popup-img" src="${p.image_url.replace(/"/g, "")}" alt="" />`
    : "";
  return `
    <div class="map-popup">
      ${img}
      <p class="map-popup-title">${escapeHtml(p.prediction || "—")}</p>
      <p class="map-popup-meta">Confidence: <strong>${conf}%</strong></p>
      <p class="map-popup-meta">${Number(p.latitude).toFixed(5)}, ${Number(p.longitude).toFixed(5)}</p>
    </div>
  `;
}

function initMap(containerId = "map-view-host") {
  mapContainerId = containerId;
  const el = getEl(containerId);
  if (!el || mapInstance) return mapInstance;
  if (!window.L) {
    throw new Error("Map library did not load. Check your internet connection.");
  }

  mapInstance = L.map(el, {
    zoomControl: true,
    scrollWheelZoom: true,
  }).setView([20, 0], 2);

  L.tileLayer("https://{s}.tile.openstreetmap.org/{z}/{x}/{y}.png", {
    attribution:
      '&copy; <a href="https://www.openstreetmap.org/copyright">OpenStreetMap</a>',
    maxZoom: 19,
  }).addTo(mapInstance);

  if (typeof L.markerClusterGroup === "function") {
    clusterGroup = L.markerClusterGroup({
      maxClusterRadius: 50,
      spiderfyOnMaxZoom: true,
      showCoverageOnHover: false,
    });
    mapInstance.addLayer(clusterGroup);
  }

  return mapInstance;
}

function clearMarkers() {
  if (!mapInstance) return;
  if (clusterGroup) {
    clusterGroup.clearLayers();
    return;
  }
  looseMarkers.forEach((m) => mapInstance.removeLayer(m));
  looseMarkers = [];
}

function addMarkers(predictions) {
  if (!mapInstance) return;

  clearMarkers();

  const bounds = [];
  predictions.forEach((p) => {
    const lat = Number(p.latitude);
    const lng = Number(p.longitude);
    if (Number.isNaN(lat) || Number.isNaN(lng)) return;

    const color = markerColor(p.prediction);
    const icon = L.divIcon({
      className: "uav-marker-wrap",
      html: `<span class="uav-marker" style="background:${color}"></span>`,
      iconSize: [20, 20],
      iconAnchor: [10, 10],
    });

    const marker = L.marker([lat, lng], { icon });
    marker.bindPopup(buildPopupHtml(p), { maxWidth: 280, className: "uav-popup" });
    bounds.push([lat, lng]);

    if (clusterGroup) clusterGroup.addLayer(marker);
    else {
      marker.addTo(mapInstance);
      looseMarkers.push(marker);
    }
  });

  if (bounds.length) {
    mapInstance.fitBounds(bounds, { padding: [40, 40], maxZoom: 14 });
  }
}

async function refreshMapMarkers(cached) {
  initMap(mapContainerId);
  let list = cached;
  if (!list) {
    const data = await fetchPredictions();
    list = data.predictions || [];
  }
  addMarkers(list);
  return list;
}

function invalidateMapSize() {
  if (mapInstance) {
    setTimeout(() => mapInstance.invalidateSize(), 200);
  }
}

async function loadHistoryTable() {
  const tbody = getEl("history-tbody");
  const err = getEl("history-error");
  showError(err, "");
  try {
    const data = await fetchPredictions();
    setHistoryData(data.predictions || [], tbody);
  } catch (e) {
    showError(err, e.message || "Failed to load history.");
  }
}

function setHistoryData(predictions, tbody) {
  historyRows = Array.isArray(predictions) ? [...predictions] : [];
  applyHistorySortFilter(tbody);
}

function bindHistoryControls(tbody, filterInput, sortSelect) {
  const rerender = () => applyHistorySortFilter(tbody);
  filterInput.addEventListener("input", rerender);
  filterInput.addEventListener("change", rerender);
  sortSelect.addEventListener("change", rerender);
}

function applyHistorySortFilter(tbody) {
  if (!tbody) return;
  const filterEl = getEl("history-filter");
  const sortEl = getEl("history-sort");
  const q = (filterEl?.value || "").trim().toLowerCase();
  const sort = sortEl?.value || "date-desc";

  let rows = [...historyRows];
  if (q) {
    rows = rows.filter((r) =>
      String(r.prediction || "")
        .toLowerCase()
        .includes(q)
    );
  }

  rows.sort((a, b) => {
    if (sort === "conf-desc")
      return Number(b.confidence) - Number(a.confidence);
    if (sort === "conf-asc")
      return Number(a.confidence) - Number(b.confidence);
    const ta = new Date(a.timestamp || 0).getTime();
    const tb = new Date(b.timestamp || 0).getTime();
    if (sort === "date-asc") return ta - tb;
    return tb - ta;
  });

  tbody.innerHTML = "";
  if (!rows.length) {
    const tr = document.createElement("tr");
    const msg =
      historyRows.length === 0
        ? "No predictions yet. Run a detection on the Dashboard."
        : "No predictions match your filters.";
    tr.innerHTML = `<td colspan="6" class="table-empty">${msg}</td>`;
    tbody.appendChild(tr);
    return;
  }

  rows.forEach((r) => {
    const tr = document.createElement("tr");
    const healthy = isHealthyLabel(r.prediction);
    tr.classList.add(healthy ? "row-healthy" : "row-risk");
    const thumb = isDataImageUrl(r.image_url)
      ? `<img class="thumb" src="${r.image_url.replace(/"/g, "")}" alt="" />`
      : '<span class="thumb thumb--empty">—</span>';
    const pct = (Number(r.confidence) * 100).toFixed(1);
    tr.innerHTML = `
      <td>${thumb}</td>
      <td><span class="badge ${healthy ? "badge--ok" : "badge--alert"}">${escapeHtml(r.prediction || "—")}</span></td>
      <td>${pct}%</td>
      <td>${Number(r.latitude).toFixed(5)}</td>
      <td>${Number(r.longitude).toFixed(5)}</td>
      <td>${escapeHtml(formatTs(r.timestamp))}</td>
    `;
    tbody.appendChild(tr);
  });
}

function initNavigation() {
  document.querySelectorAll("[data-nav]").forEach((btn) => {
    btn.addEventListener("click", () => {
      const tab = btn.getAttribute("data-nav");
      if (tab) setActiveTab(tab);
    });
  });
}

function initDashboardForm() {
  const form = getEl("predict-form");
  const zone = getEl("drop-zone");
  const fileInput = getEl("image-input");
  const preview = getEl("image-preview");
  const submitBtn = getEl("submit-btn");
  const resultCard = getEl("result-card");
  const formError = getEl("form-error");

  let lastPreviewUrl = null;

  setupDropZone(zone, fileInput, preview, () => {
    showError(formError, "");
    hidePredictionCard(resultCard);
  });

  if (!form) return;
  form.addEventListener("submit", async (e) => {
    e.preventDefault();
    showError(formError, "");

    const file = fileInput.files?.[0];
    const lat = getEl("lat-input")?.value?.trim();
    const lng = getEl("lng-input")?.value?.trim();

    if (!file) {
      showError(formError, "Please choose an image to analyze.");
      return;
    }
    if (lat === "" || lng === "") {
      showError(formError, "Latitude and longitude are required.");
      return;
    }

    const fd = new FormData();
    fd.append("file", file);
    fd.append("latitude", lat);
    fd.append("longitude", lng);

    setSubmitLoading(submitBtn, true);
    try {
      const res = await postPredict(fd);
      if (lastPreviewUrl) URL.revokeObjectURL(lastPreviewUrl);
      lastPreviewUrl = URL.createObjectURL(file);
      renderPredictionCard(resultCard, {
        ...res,
        previewUrl: lastPreviewUrl,
      });

      const tbody = getEl("history-tbody");
      if (tbody && !getEl("panel-history").hidden) {
        const data = await fetchPredictions();
        setHistoryData(data.predictions || [], tbody);
      }
    } catch (err) {
      showError(formError, err.message || "Prediction failed. Is the API running?");
    } finally {
      setSubmitLoading(submitBtn, false);
    }
  });
}

function computeMotorDuties() {
  if (!controlState.armed) return [0, 0, 0, 0];

  const base = 1000 + controlState.throttle * 1000;
  const roll = clamp(controlState.orientation.roll / 45, -1, 1) * 80;
  const pitch = clamp(controlState.orientation.pitch / 45, -1, 1) * 80;
  const yaw = clamp(controlState.orientation.yaw / 90, -1, 1) * 60;

  return [
    base - roll + pitch + yaw,
    base + roll + pitch - yaw,
    base - roll - pitch - yaw,
    base + roll - pitch + yaw,
  ].map((value) => Math.round(clamp(value, 1000, 2000)));
}

function renderTelemetry() {
  const throttleOutput = getEl("base-throttle-value");
  if (throttleOutput) throttleOutput.textContent = controlState.throttle.toFixed(2);

  const armBtn = getEl("arm-toggle");
  if (armBtn) {
    armBtn.textContent = controlState.armed ? "Armed" : "Disarmed";
    armBtn.setAttribute("aria-pressed", String(controlState.armed));
    armBtn.classList.toggle("btn--success", controlState.armed);
    armBtn.classList.toggle("btn--danger", !controlState.armed);
  }

  const { roll, pitch, yaw } = controlState.orientation;
  const motors = computeMotorDuties();
  const attitude = getEl("telemetry-attitude");
  const motorEl = getEl("telemetry-motors");
  const armedEl = getEl("telemetry-armed");
  const tickEl = getEl("telemetry-tick");

  if (attitude) attitude.textContent = `${roll.toFixed(1)}, ${pitch.toFixed(1)}, ${yaw.toFixed(1)}`;
  if (motorEl) {
    motorEl.textContent = `FL: ${motors[0]}, FR: ${motors[1]}, RL: ${motors[2]}, RR: ${motors[3]}`;
  }
  if (armedEl) armedEl.textContent = controlState.armed ? "Yes" : "No";
  if (tickEl) tickEl.textContent = String(controlState.tick);
}

function syncControlStateFromInputs() {
  controlState.throttle = clamp(readNumber("base-throttle"), 0, 1);
  controlState.position.x = readNumber("pos-x");
  controlState.position.y = readNumber("pos-y");
  controlState.position.z = readNumber("pos-z");
  controlState.orientation.roll = readNumber("ori-roll");
  controlState.orientation.pitch = readNumber("ori-pitch");
  controlState.orientation.yaw = readNumber("ori-yaw");
}

function setPositionInputs() {
  const x = getEl("pos-x");
  const y = getEl("pos-y");
  const z = getEl("pos-z");
  if (x) x.value = controlState.position.x.toFixed(1);
  if (y) y.value = controlState.position.y.toFixed(1);
  if (z) z.value = controlState.position.z.toFixed(1);
}

function bumpTick() {
  controlState.tick = (controlState.tick + 1) >>> 0;
}

function applyManualCommand(command) {
  const step = 1;
  if (command === "forward") controlState.position.y += step;
  if (command === "backward") controlState.position.y -= step;
  if (command === "left") controlState.position.x -= step;
  if (command === "right") controlState.position.x += step;
  setPositionInputs();
  bumpTick();
  renderTelemetry();
}

function initUavControls() {
  const armBtn = getEl("arm-toggle");
  const throttle = getEl("base-throttle");

  armBtn?.addEventListener("click", () => {
    controlState.armed = !controlState.armed;
    bumpTick();
    renderTelemetry();
  });

  throttle?.addEventListener("input", () => {
    syncControlStateFromInputs();
    bumpTick();
    renderTelemetry();
  });

  ["pos-x", "pos-y", "pos-z", "ori-roll", "ori-pitch", "ori-yaw"].forEach((id) => {
    getEl(id)?.addEventListener("input", () => {
      syncControlStateFromInputs();
      bumpTick();
      renderTelemetry();
    });
  });

  document.querySelectorAll("[data-manual]").forEach((btn) => {
    btn.addEventListener("click", () => applyManualCommand(btn.dataset.manual));
  });

  syncControlStateFromInputs();
  renderTelemetry();
  setInterval(() => {
    bumpTick();
    renderTelemetry();
  }, 1000);
}

function initHistoryFilters() {
  const tbody = getEl("history-tbody");
  const filter = getEl("history-filter");
  const sort = getEl("history-sort");
  if (tbody && filter && sort) bindHistoryControls(tbody, filter, sort);
}

function initLucide() {
  if (window.lucide && window.lucide.createIcons) {
    window.lucide.createIcons();
  }
}

document.addEventListener("DOMContentLoaded", () => {
  initLucide();
  initNavigation();
  initDashboardForm();
  initUavControls();
  initHistoryFilters();
  setActiveTab("dashboard");
});