<script lang="ts">
    import { publish, TOPICS } from "../lib/mqtt";
    import { clamp } from "../lib/utils";
    import type { ControlState, ManualCommand } from "../lib/types";

    let armed = $state(false);
    let throttle = $state(0);
    let posX = $state(0);
    let posY = $state(0);
    let posZ = $state(0);
    let roll = $state(0);
    let pitch = $state(0);
    let yaw = $state(0);
    let tick = $state(0);

    function bumpTick() {
        tick = (tick + 1) >>> 0;
    }

    function buildState(): ControlState {
        return {
            armed,
            throttle,
            position: { x: posX, y: posY, z: posZ },
            orientation: { roll, pitch, yaw },
            tick,
        };
    }

    function sendControl() {
        bumpTick();
        publish(TOPICS.CONTROL, buildState());
    }

    function toggleArm() {
        armed = !armed;
        sendControl();
    }

    function onThrottle(e: Event) {
        throttle = clamp(Number((e.target as HTMLInputElement).value), 0, 1);
        sendControl();
    }

    function onInput() {
        sendControl();
    }

    function applyManual(cmd: ManualCommand) {
        const step = 1;
        if (cmd === "forward") posY += step;
        if (cmd === "backward") posY -= step;
        if (cmd === "left") posX -= step;
        if (cmd === "right") posX += step;
        sendControl();
    }

    let motors = $derived.by(() => {
        if (!armed) return [0, 0, 0, 0] as const;
        const base = 1000 + throttle * 1000;
        const r = clamp(roll / 45, -1, 1) * 80;
        const p = clamp(pitch / 45, -1, 1) * 80;
        const y = clamp(yaw / 90, -1, 1) * 60;
        return [
            Math.round(clamp(base - r + p + y, 1000, 2000)),
            Math.round(clamp(base + r + p - y, 1000, 2000)),
            Math.round(clamp(base - r - p - y, 1000, 2000)),
            Math.round(clamp(base + r - p + y, 1000, 2000)),
        ] as const;
    });
</script>

<article class="card card--controls">
    <div class="card__head">
        <h3 class="card__title">UAV controls</h3>
        <p class="card__desc">
            Send basic flight commands and preview the local control state.
        </p>
    </div>

    <div class="control-stack">
        <button
            type="button"
            class="btn"
            class:btn--danger={!armed}
            class:btn--success={armed}
            aria-pressed={armed}
            onclick={toggleArm}
        >
            {armed ? "Armed" : "Disarmed"}
        </button>

        <label class="field">
            <span class="field__label">Base throttle</span>
            <div class="range-row">
                <input
                    type="range"
                    min="0"
                    max="1"
                    step="0.01"
                    value={throttle}
                    class="range"
                    oninput={onThrottle}
                />
                <output class="mono">{throttle.toFixed(2)}</output>
            </div>
        </label>

        <div class="field-group">
            <p class="field-group__title">Position</p>
            <div class="field-row field-row--triple">
                <label class="field">
                    <span class="field__label">X</span>
                    <input
                        bind:value={posX}
                        type="number"
                        step="0.1"
                        class="field__input mono"
                        oninput={onInput}
                    />
                </label>
                <label class="field">
                    <span class="field__label">Y</span>
                    <input
                        bind:value={posY}
                        type="number"
                        step="0.1"
                        class="field__input mono"
                        oninput={onInput}
                    />
                </label>
                <label class="field">
                    <span class="field__label">Z</span>
                    <input
                        bind:value={posZ}
                        type="number"
                        step="0.1"
                        class="field__input mono"
                        oninput={onInput}
                    />
                </label>
            </div>
        </div>

        <div class="field-group">
            <p class="field-group__title">Orientation</p>
            <div class="field-row field-row--triple">
                <label class="field">
                    <span class="field__label">Roll</span>
                    <input
                        bind:value={roll}
                        type="number"
                        step="0.1"
                        class="field__input mono"
                        oninput={onInput}
                    />
                </label>
                <label class="field">
                    <span class="field__label">Pitch</span>
                    <input
                        bind:value={pitch}
                        type="number"
                        step="0.1"
                        class="field__input mono"
                        oninput={onInput}
                    />
                </label>
                <label class="field">
                    <span class="field__label">Yaw</span>
                    <input
                        bind:value={yaw}
                        type="number"
                        step="0.1"
                        class="field__input mono"
                        oninput={onInput}
                    />
                </label>
            </div>
        </div>

        <div class="manual-pad" aria-label="Manual controls">
            <button
                type="button"
                class="manual-btn manual-btn--up"
                aria-label="Forward"
                onclick={() => applyManual("forward")}>↑</button
            >
            <button
                type="button"
                class="manual-btn manual-btn--left"
                aria-label="Left"
                onclick={() => applyManual("left")}>←</button
            >
            <button
                type="button"
                class="manual-btn manual-btn--right"
                aria-label="Right"
                onclick={() => applyManual("right")}>→</button
            >
            <button
                type="button"
                class="manual-btn manual-btn--down"
                aria-label="Backward"
                onclick={() => applyManual("backward")}>↓</button
            >
        </div>
    </div>
</article>
