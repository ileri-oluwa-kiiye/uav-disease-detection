import { postPredict, fetchPredictions } from "./api.js";
import {
  setupDropZone,
  setSubmitLoading,
  renderPredictionCard,
  hidePredictionCard,
  showError,
  setHistoryData,
  bindHistoryControls,
} from "./ui.js";
import { initMap, refreshMapMarkers, invalidateMapSize } from "./map.js";

const TABS = ["dashboard", "map", "history"];



const TOPBAR_COPY = {
  dashboard: {
    title: "Mission dashboard",
    subtitle:
      "Upload field imagery, run inference, and review geotagged detections.",
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

function getEl(id) {
  return document.getElementById(id);
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

    zone.addEventListener("keydown", (e) => {
      if (e.key === "Enter" || e.key === " ") {
        e.preventDefault();
        fileInput.click();
      }
    });
  });

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
  initHistoryFilters();
  setActiveTab("dashboard");
});


console.log("JS loaded");