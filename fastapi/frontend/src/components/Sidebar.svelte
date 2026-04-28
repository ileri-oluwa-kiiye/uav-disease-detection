<script lang="ts">
    import { LayoutDashboard, Map, History } from "lucide-svelte";
    import type { Tab } from "../lib/types";

    interface Props {
        activeTab: Tab;
        onNavigate: (tab: Tab) => void;
    }

    let { activeTab, onNavigate }: Props = $props();

    const navItems: {
        tab: Tab;
        label: string;
        icon: typeof LayoutDashboard;
    }[] = [
        { tab: "dashboard", label: "Dashboard", icon: LayoutDashboard },
        { tab: "map", label: "Map View", icon: Map },
        { tab: "history", label: "History", icon: History },
    ];
</script>

<aside class="sidebar" aria-label="Main navigation">
    <div class="sidebar__brand">
        <span class="sidebar__logo" aria-hidden="true"></span>
        <div>
            <h1 class="sidebar__title">AgroAI Monitor</h1>
            <p class="sidebar__tagline">UAV crop intelligence</p>
        </div>
    </div>

    <nav class="sidebar__nav" aria-label="Sections">
        {#each navItems as { tab, label, icon: Icon }}
            <button
                type="button"
                class="nav-item"
                class:nav-item--active={activeTab === tab}
                onclick={() => onNavigate(tab)}
            >
                <Icon class="nav-item__icon" size={18} />
                <span>{label}</span>
            </button>
        {/each}
    </nav>

    <div class="sidebar__footer">
        <p class="sidebar__hint">
            API: <code class="mono">127.0.0.1:8000</code>
        </p>
    </div>
</aside>
