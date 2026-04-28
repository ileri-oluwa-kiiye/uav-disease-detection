<script lang="ts">
    import { onMount } from "svelte";
    import Sidebar from "./components/Sidebar.svelte";
    import Topbar from "./components/Topbar.svelte";
    import ImageUpload from "./components/ImageUpload.svelte";
    import UavControls from "./components/UavControls.svelte";
    import Telemetry from "./components/Telemetry.svelte";
    import ResultCard from "./components/ResultCard.svelte";
    import MapView from "./components/MapView.svelte";
    import History from "./components/History.svelte";
    import { connect, disconnect } from "./lib/mqtt";
    import type { Tab, PredictResponse } from "./lib/types";

    let activeTab: Tab = $state("dashboard");
    let latestResult: (PredictResponse & { previewUrl: string }) | null =
        $state(null);

    function onNavigate(tab: Tab) {
        activeTab = tab;
    }

    function onResult(result: PredictResponse & { previewUrl: string }) {
        latestResult = result;
    }

    onMount(() => {
        const brokerUrl =
            import.meta.env.VITE_MQTT_BROKER_URL ?? "ws://localhost:9001";
        connect({ brokerUrl });
        return () => disconnect();
    });
</script>

<div class="app">
    <Sidebar {activeTab} {onNavigate} />

    <div class="main-wrap">
        <main class="main">
            <Topbar {activeTab} />

            {#if activeTab === "dashboard"}
                <section class="panel" aria-label="Dashboard">
                    <div class="grid-2">
                        <ImageUpload {onResult} />
                        <UavControls />
                        <Telemetry />
                        <ResultCard result={latestResult} />
                    </div>
                </section>
            {:else if activeTab === "map"}
                <section class="panel panel--map" aria-label="Map view">
                    <MapView />
                </section>
            {:else if activeTab === "history"}
                <section class="panel" aria-label="Prediction history">
                    <History />
                </section>
            {/if}
        </main>
    </div>
</div>
