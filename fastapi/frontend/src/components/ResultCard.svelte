<script lang="ts">
    import { isHealthyLabel, formatTs } from "../lib/utils";
    import type { PredictResponse } from "../lib/types";

    interface Props {
        result: (PredictResponse & { previewUrl?: string }) | null;
    }

    let { result }: Props = $props();

    let healthy = $derived(result ? isHealthyLabel(result.prediction) : false);
    let pct = $derived(
        result ? Math.round(Number(result.confidence) * 1000) / 10 : 0,
    );
</script>

{#if result}
    <article
        class="card card--result"
        class:result-card--healthy={healthy}
        class:result-card--risk={!healthy}
    >
        <div class="card__head">
            <h3 class="card__title">Latest result</h3>
            <p class="card__desc">Model output for the current upload.</p>
        </div>

        <div class="result-body">
            <div class="result-visual">
                {#if result.previewUrl}
                    <img class="result-img" src={result.previewUrl} alt="" />
                {/if}
            </div>
            <div class="result-meta">
                <p class="result-label">Disease class</p>
                <p class="result-class mono">{result.prediction || "—"}</p>
                <p class="result-label">Confidence</p>
                <div class="conf-track">
                    <div
                        class="conf-bar"
                        style:width="{Math.min(100, Math.max(0, pct))}%"
                    ></div>
                </div>
                <p class="conf-pct mono">{pct}%</p>
                <p class="result-label">Coordinates</p>
                <p class="result-coords mono">
                    {Number(result.latitude).toFixed(5)}, {Number(
                        result.longitude,
                    ).toFixed(5)}
                </p>
                <p class="result-label">Timestamp</p>
                <p class="result-time mono">
                    {result.timestamp ? formatTs(result.timestamp) : "—"}
                </p>
            </div>
        </div>
    </article>
{/if}
