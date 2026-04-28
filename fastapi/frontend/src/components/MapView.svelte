<script lang="ts">
    import { onMount } from "svelte";
    import { fetchPredictions } from "../lib/api";
    import { isHealthyLabel, isDataImageUrl } from "../lib/utils";
    import type { Prediction } from "../lib/types";
    import type L from "leaflet";

    let mapEl: HTMLDivElement;
    let mapInstance: L.Map | null = null;
    let clusterGroup: any = null;
    let error = $state("");

    function markerColor(prediction: string): string {
        return isHealthyLabel(prediction) ? "#22c55e" : "#ef4444";
    }

    function buildPopupHtml(p: Prediction): string {
        const conf = (Number(p.confidence) * 100).toFixed(1);
        const img = isDataImageUrl(p.image_url)
            ? `<img class="map-popup-img" src="${p.image_url!.replace(/"/g, "")}" alt="" />`
            : "";
        return `
      <div class="map-popup">
        ${img}
        <p class="map-popup-title">${p.prediction || "—"}</p>
        <p class="map-popup-meta">Confidence: <strong>${conf}%</strong></p>
        <p class="map-popup-meta">${Number(p.latitude).toFixed(5)}, ${Number(p.longitude).toFixed(5)}</p>
      </div>
    `;
    }

    async function initAndLoad() {
        const L = await import("leaflet");
        await import("leaflet/dist/leaflet.css");

        if (mapInstance) {
            mapInstance.invalidateSize();
        } else {
            mapInstance = L.map(mapEl, {
                zoomControl: true,
                scrollWheelZoom: true,
            }).setView([20, 0], 2);

            L.tileLayer("https://{s}.tile.openstreetmap.org/{z}/{x}/{y}.png", {
                attribution:
                    '&copy; <a href="https://www.openstreetmap.org/copyright">OpenStreetMap</a>',
                maxZoom: 19,
            }).addTo(mapInstance);
        }

        try {
            error = "";
            const data = await fetchPredictions();
            addMarkers(L, data.predictions || []);
        } catch (e: any) {
            error = e.message || "Could not load predictions.";
        }
    }

    function addMarkers(
        L: typeof import("leaflet"),
        predictions: Prediction[],
    ) {
        if (!mapInstance) return;

        const bounds: L.LatLngTuple[] = [];

        for (const p of predictions) {
            const lat = Number(p.latitude);
            const lng = Number(p.longitude);
            if (Number.isNaN(lat) || Number.isNaN(lng)) continue;

            const color = markerColor(p.prediction);
            const icon = L.divIcon({
                className: "uav-marker-wrap",
                html: `<span class="uav-marker" style="background:${color}"></span>`,
                iconSize: [20, 20],
                iconAnchor: [10, 10],
            });

            const marker = L.marker([lat, lng], { icon });
            marker.bindPopup(buildPopupHtml(p), {
                maxWidth: 280,
                className: "uav-popup",
            });
            bounds.push([lat, lng]);
            marker.addTo(mapInstance!);
        }

        if (bounds.length) {
            mapInstance.fitBounds(bounds, { padding: [40, 40], maxZoom: 14 });
        }
    }

    onMount(() => {
        initAndLoad();
        return () => {
            if (mapInstance) {
                mapInstance.remove();
                mapInstance = null;
            }
        };
    });
</script>

<article class="card card--map">
    <div class="card__head card__head--row">
        <div>
            <h3 class="card__title">Geospatial detections</h3>
            <p class="card__desc">
                Markers cluster by region; click for image and scores.
            </p>
        </div>
        <div class="legend">
            <span class="legend__dot legend__dot--ok"></span> Healthy
            <span class="legend__dot legend__dot--risk"></span> Infected / stress
        </div>
    </div>

    {#if error}
        <div class="alert alert--map" role="alert">{error}</div>
    {/if}

    <div
        bind:this={mapEl}
        class="map-host"
        role="application"
        aria-label="Leaflet map"
    ></div>
</article>
