const DROP_ACTIVE = "drop-zone--active";

export function isHealthyLabel(prediction) {
  if (!prediction) return false;
  return String(prediction).toLowerCase().includes("healthy");
}

/**
 * @param {HTMLElement} zone
 * @param {HTMLInputElement} fileInput
 * @param {HTMLImageElement} preview
 * @param {(file: File) => void} [onFile]
 */
export function setupDropZone(zone, fileInput, preview, onFile) {
  const activate = () => zone.classList.add(DROP_ACTIVE);
  const deactivate = () => zone.classList.remove(DROP_ACTIVE);

  zone.addEventListener("click", () => fileInput.click());

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

/**
 * @param {HTMLElement} btn
 * @param {boolean} loading
 */
export function setSubmitLoading(btn, loading) {
  btn.disabled = loading;
  btn.classList.toggle("btn--loading", loading);
  const label = btn.querySelector(".btn-label");
  const spin = btn.querySelector(".btn-spinner");
  if (label) label.hidden = loading;
  if (spin) spin.hidden = !loading;
}

/**
 * @param {HTMLElement} card
 * @param {object} payload — prediction response + optional image Object URL
 */
export function renderPredictionCard(card, payload) {
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
  if (confText) confText.textContent = `${pct}%`;
  if (coords)
    coords.textContent = `${Number(payload.latitude).toFixed(5)}, ${Number(payload.longitude).toFixed(5)}`;
  if (timeEl) timeEl.textContent = payload.timestamp ? formatTs(payload.timestamp) : "—";

  if (img && payload.previewUrl) {
    img.src = payload.previewUrl;
    img.hidden = false;
  }
}

export function hidePredictionCard(card) {
  card.hidden = true;
}

/**
 * @param {HTMLElement} el
 * @param {string} message — empty to clear
 */
export function showError(el, message) {
  if (!el) return;
  el.textContent = message || "";
  el.hidden = !message;
  el.classList.toggle("alert--visible", !!message);
}

function isDataImageUrl(u) {
  return (
    typeof u === "string" &&
    /^data:image\/(jpeg|jpg|png|webp);base64,/i.test(u)
  );
}

function formatTs(iso) {
  try {
    const d = new Date(iso);
    return d.toLocaleString();
  } catch {
    return iso;
  }
}

let historyRows = [];

/**
 * @param {object[]} predictions
 * @param {HTMLTableSectionElement} tbody
 */
export function setHistoryData(predictions, tbody) {
  historyRows = Array.isArray(predictions) ? [...predictions] : [];
  applyHistorySortFilter(tbody);
}

/**
 * @param {HTMLTableSectionElement} tbody
 * @param {HTMLInputElement} filterInput
 * @param {HTMLSelectElement} sortSelect
 */
export function bindHistoryControls(tbody, filterInput, sortSelect) {
  const rerender = () => applyHistorySortFilter(tbody);
  filterInput.addEventListener("input", rerender);
  filterInput.addEventListener("change", rerender);
  sortSelect.addEventListener("change", rerender);
}

function applyHistorySortFilter(tbody) {
  const filterEl = document.getElementById("history-filter");
  const sortEl = document.getElementById("history-sort");
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

function escapeHtml(s) {
  const div = document.createElement("div");
  div.textContent = s == null ? "" : String(s);
  return div.innerHTML;
}

console.log("JS loaded");