import { fetchPredictions } from "./api.js";

let mapInstance = null;
let clusterGroup = null;
let mapContainerId = "map-view-host";
let looseMarkers = [];

function isHealthyLabel(prediction) {
  if (!prediction) return false;
  return String(prediction).toLowerCase().includes("healthy");
}

function markerColor(prediction) {
  return isHealthyLabel(prediction) ? "#22c55e" : "#ef4444";
}

function escapeHtml(s) {
  const div = document.createElement("div");
  div.textContent = s;
  return div.innerHTML;
}

function isDataImageUrl(u) {
  return (
    typeof u === "string" &&
    /^data:image\/(jpeg|jpg|png|webp);base64,/i.test(u)
  );
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

/**
 * @param {string} containerId
 */
export function initMap(containerId = "map-view-host") {
  mapContainerId = containerId;
  const el = document.getElementById(containerId);
  if (!el || mapInstance) return mapInstance;

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

/**
 * Load markers from GET /predictions (or use cached list).
 * @param {object[]} [cached]
 */
export async function refreshMapMarkers(cached) {
  initMap(mapContainerId);
  let list = cached;
  if (!list) {
    const data = await fetchPredictions();
    list = data.predictions || [];
  }
  addMarkers(list);
  return list;
}

export function invalidateMapSize() {
  if (mapInstance) {
    setTimeout(() => mapInstance.invalidateSize(), 200);
  }
}
console.log("Map JS loaded");