<script lang="ts">
    import { connectionStatus } from "../lib/mqtt";
    import type { Tab } from "../lib/types";

    interface Props {
        activeTab: Tab;
    }

    let { activeTab }: Props = $props();

    const copy: Record<Tab, { title: string; subtitle: string }> = {
        dashboard: {
            title: "Mission dashboard",
            subtitle:
                "Control the UAV, monitor telemetry, upload field imagery, and review detections.",
        },
        map: {
            title: "Map view",
            subtitle:
                "Clustered markers from GET /predictions — click for full context.",
        },
        history: {
            title: "Flight history",
            subtitle:
                "Sort and filter past runs for reporting and traceability.",
        },
    };

    let current = $derived(copy[activeTab]);
    let status = $derived($connectionStatus);
</script>

<header class="topbar">
    <div>
        <h2 class="topbar__title">{current.title}</h2>
        <p class="topbar__subtitle">{current.subtitle}</p>
    </div>
    <div class="topbar__pill" role="status">
        <span
            class="pulse"
            class:pulse--disconnected={status === "disconnected"}
            class:pulse--connecting={status === "connecting"}
            aria-hidden="true"
        ></span>
        {#if status === "connected"}
            Live uplink
        {:else if status === "connecting"}
            Connecting…
        {:else}
            Offline
        {/if}
    </div>
</header>

<style>
    .pulse--disconnected {
        background: var(--danger);
        box-shadow: none;
        animation: none;
    }
    .pulse--connecting {
        background: var(--warn);
        animation: pulse 1s ease-out infinite;
    }
</style>
