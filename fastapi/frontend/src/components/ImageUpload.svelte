<script lang="ts">
    import { ImagePlus } from "lucide-svelte";
    import { postPredict } from "../lib/api";
    import type { PredictResponse } from "../lib/types";

    interface Props {
        onResult: (result: PredictResponse & { previewUrl: string }) => void;
    }

    let { onResult }: Props = $props();

    let fileInput: HTMLInputElement;
    let selectedFile: File | null = $state(null);
    let previewUrl: string | null = $state(null);
    let latitude = $state("");
    let longitude = $state("");
    let error = $state("");
    let loading = $state(false);
    let dropActive = $state(false);

    function handleFile(file: File) {
        if (!file.type.startsWith("image/")) return;
        selectedFile = file;
        if (previewUrl) URL.revokeObjectURL(previewUrl);
        previewUrl = URL.createObjectURL(file);
        error = "";
    }

    function onDrop(e: DragEvent) {
        e.preventDefault();
        dropActive = false;
        const f = e.dataTransfer?.files?.[0];
        if (f) handleFile(f);
    }

    function onFileChange() {
        const f = fileInput?.files?.[0];
        if (f) handleFile(f);
    }

    async function onSubmit(e: SubmitEvent) {
        e.preventDefault();
        error = "";

        if (!selectedFile) {
            error = "Please choose an image to analyze.";
            return;
        }
        if (!latitude.trim() || !longitude.trim()) {
            error = "Latitude and longitude are required.";
            return;
        }

        const fd = new FormData();
        fd.append("file", selectedFile);
        fd.append("latitude", latitude.trim());
        fd.append("longitude", longitude.trim());

        loading = true;
        try {
            const res = await postPredict(fd);
            const url = URL.createObjectURL(selectedFile);
            onResult({ ...res, previewUrl: url });
        } catch (err: any) {
            error = err.message || "Prediction failed. Is the API running?";
        } finally {
            loading = false;
        }
    }
</script>

<article class="card card--upload">
    <div class="card__head">
        <h3 class="card__title">New analysis</h3>
        <p class="card__desc">Drop a canopy image and set GPS coordinates.</p>
    </div>

    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
        class="drop-zone"
        class:drop-zone--active={dropActive}
        role="button"
        tabindex="0"
        aria-label="Upload image by click or drag and drop"
        onclick={() => fileInput.click()}
        onkeydown={(e) => {
            if (e.key === "Enter" || e.key === " ") {
                e.preventDefault();
                fileInput.click();
            }
        }}
        ondragenter={(e) => {
            e.preventDefault();
            dropActive = true;
        }}
        ondragover={(e) => {
            e.preventDefault();
            dropActive = true;
        }}
        ondragleave={(e) => {
            e.preventDefault();
            const zone = e.currentTarget as HTMLElement;
            if (!zone.contains(e.relatedTarget as Node)) dropActive = false;
        }}
        ondrop={onDrop}
    >
        <input
            bind:this={fileInput}
            type="file"
            accept="image/*"
            aria-hidden="true"
            style="display:none"
            onchange={onFileChange}
        />
        <ImagePlus class="drop-zone__icon" size={36} />
        <p class="drop-zone__text">
            <strong>Drag & drop</strong> or click to browse
        </p>
        <p class="drop-zone__hint">PNG, JPG, WEBP — field or UAV still</p>
    </div>

    {#if previewUrl}
        <div class="preview-wrap preview-wrap--visible">
            <img class="preview-img" src={previewUrl} alt="Selected preview" />
        </div>
    {/if}

    <form class="coords-form" novalidate onsubmit={onSubmit}>
        <div class="field-row">
            <label class="field">
                <span class="field__label">Latitude</span>
                <input
                    bind:value={latitude}
                    type="text"
                    inputmode="decimal"
                    class="field__input mono"
                    placeholder="e.g. 6.5244"
                    autocomplete="off"
                />
            </label>
            <label class="field">
                <span class="field__label">Longitude</span>
                <input
                    bind:value={longitude}
                    type="text"
                    inputmode="decimal"
                    class="field__input mono"
                    placeholder="e.g. 3.3792"
                    autocomplete="off"
                />
            </label>
        </div>

        {#if error}
            <div class="alert" role="alert">{error}</div>
        {/if}

        <button
            type="submit"
            class="btn btn--primary"
            class:btn--loading={loading}
            disabled={loading}
        >
            {#if loading}
                <span class="btn-spinner" aria-hidden="true"></span>
            {:else}
                <span class="btn-label">Run detection</span>
            {/if}
        </button>
    </form>
</article>

<style>
    .preview-img {
        display: block;
    }
</style>
