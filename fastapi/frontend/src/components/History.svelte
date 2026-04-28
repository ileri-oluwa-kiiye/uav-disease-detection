<script lang="ts">
    import { onMount } from "svelte";
    import { Search } from "lucide-svelte";
    import { fetchPredictions } from "../lib/api";
    import { isHealthyLabel, isDataImageUrl, formatTs } from "../lib/utils";
    import type { Prediction } from "../lib/types";

    let predictions: Prediction[] = $state([]);
    let filterQuery = $state("");
    let sortOrder = $state("date-desc");
    let error = $state("");

    let filtered = $derived.by(() => {
        const q = filterQuery.trim().toLowerCase();
        let rows = [...predictions];

        if (q) {
            rows = rows.filter((r) =>
                String(r.prediction || "")
                    .toLowerCase()
                    .includes(q),
            );
        }

        rows.sort((a, b) => {
            if (sortOrder === "conf-desc")
                return Number(b.confidence) - Number(a.confidence);
            if (sortOrder === "conf-asc")
                return Number(a.confidence) - Number(b.confidence);
            const ta = new Date(a.timestamp || 0).getTime();
            const tb = new Date(b.timestamp || 0).getTime();
            if (sortOrder === "date-asc") return ta - tb;
            return tb - ta;
        });

        return rows;
    });

    let emptyMessage = $derived(
        predictions.length === 0
            ? "No predictions yet. Run a detection on the Dashboard."
            : "No predictions match your filters.",
    );

    onMount(async () => {
        try {
            error = "";
            const data = await fetchPredictions();
            predictions = data.predictions || [];
        } catch (e: any) {
            error = e.message || "Failed to load history.";
        }
    });

    function pct(confidence: number): string {
        return (Number(confidence) * 100).toFixed(1);
    }
</script>

<article class="card">
    <div class="card__head card__head--row card__head--wrap">
        <div>
            <h3 class="card__title">Flight history</h3>
            <p class="card__desc">
                Search by class, sort by confidence or time.
            </p>
        </div>
        <div class="history-tools">
            <label class="search-field">
                <Search class="search-field__icon" size={16} />
                <input
                    bind:value={filterQuery}
                    type="search"
                    class="search-field__input"
                    placeholder="Filter by disease class…"
                    autocomplete="off"
                />
            </label>
            <select
                bind:value={sortOrder}
                class="select"
                aria-label="Sort predictions"
            >
                <option value="date-desc">Newest first</option>
                <option value="date-asc">Oldest first</option>
                <option value="conf-desc">Confidence high → low</option>
                <option value="conf-asc">Confidence low → high</option>
            </select>
        </div>
    </div>

    {#if error}
        <div class="alert" role="alert">{error}</div>
    {/if}

    <div class="table-scroll">
        <table class="data-table">
            <thead>
                <tr>
                    <th scope="col">Image</th>
                    <th scope="col">Class</th>
                    <th scope="col">Confidence</th>
                    <th scope="col">Latitude</th>
                    <th scope="col">Longitude</th>
                    <th scope="col">Timestamp</th>
                </tr>
            </thead>
            <tbody>
                {#if filtered.length === 0}
                    <tr>
                        <td colspan="6" class="table-empty">{emptyMessage}</td>
                    </tr>
                {:else}
                    {#each filtered as row}
                        {@const healthy = isHealthyLabel(row.prediction)}
                        <tr class={healthy ? "row-healthy" : "row-risk"}>
                            <td>
                                {#if isDataImageUrl(row.image_url)}
                                    <img
                                        class="thumb"
                                        src={row.image_url}
                                        alt=""
                                    />
                                {:else}
                                    <span class="thumb thumb--empty">—</span>
                                {/if}
                            </td>
                            <td>
                                <span
                                    class="badge"
                                    class:badge--ok={healthy}
                                    class:badge--alert={!healthy}
                                >
                                    {row.prediction || "—"}
                                </span>
                            </td>
                            <td>{pct(row.confidence)}%</td>
                            <td>{Number(row.latitude).toFixed(5)}</td>
                            <td>{Number(row.longitude).toFixed(5)}</td>
                            <td>{formatTs(row.timestamp)}</td>
                        </tr>
                    {/each}
                {/if}
            </tbody>
        </table>
    </div>
</article>
