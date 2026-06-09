<script lang="ts">
    import { onMount, onDestroy } from "svelte";
    import { publish, TOPICS } from "../lib/mqtt";
    import { clamp } from "../lib/utils";
    import type { ControlState } from "../lib/types";

    type Mode = "manual" | "auto";
    let mode = $state<Mode>("manual");

    // shared
    let armed = $state(false);
    let throttle = $state(0);
    let yaw = $state(0);
    let tick = $state(0);

    // --- manual (WASD/arrows as held sticks) ---
    // deflection magnitude applied while a key is held, in degrees of desired tilt
    const TILT = 12; // max commanded roll/pitch angle
    const YAW_STEP = 30; // yaw rate-ish nudge per held tick (deg)
    const SEND_HZ = 20; // manual stream rate

    let pitchCmd = $state(0); // forward/back  (W/S or Up/Down)
    let rollCmd = $state(0); // left/right    (A/D or Left/Right)
    const held = new Set<string>();
    let manualTimer: ReturnType<typeof setInterval> | null = null;

    // --- semi-autonomous (position delta -> drone/goto) ---
    let stepMeters = $state(1.0); // how far each goto nudge moves
    let altDelta = $state(0); // z delta for goto

    function bumpTick() {
        tick = (tick + 1) >>> 0;
    }

    // ---------- MANUAL ----------
    function recomputeSticks() {
        // arrows/WASD set discrete deflection; opposing keys cancel
        const fwd = held.has("w") || held.has("arrowup");
        const back = held.has("s") || held.has("arrowdown");
        const left = held.has("a") || held.has("arrowleft");
        const right = held.has("d") || held.has("arrowright");
        pitchCmd = (fwd ? TILT : 0) - (back ? TILT : 0);
        rollCmd = (right ? TILT : 0) - (left ? TILT : 0);
    }

    function sendManual() {
        bumpTick();
        const state: ControlState = {
            armed,
            throttle,
            position: { x: 0, y: 0, z: 0 }, // unused in manual
            orientation: { roll: rollCmd, pitch: pitchCmd, yaw },
            tick,
        };
        publish(TOPICS.CONTROL, state);
    }

    function startManualStream() {
        if (manualTimer) return;
        // stream at fixed rate so the STM sees fresh RC frames (asserts Manual mode)
        manualTimer = setInterval(() => {
            if (pitchCmd !== 0 || rollCmd !== 0 || held.size > 0) sendManual();
        }, 1000 / SEND_HZ);
    }
    function stopManualStream() {
        if (manualTimer) {
            clearInterval(manualTimer);
            manualTimer = null;
        }
        pitchCmd = 0;
        rollCmd = 0;
        held.clear();
    }

    function onKeyDown(e: KeyboardEvent) {
        if (mode !== "manual") return;
        const k = e.key.toLowerCase();
        if (
            ![
                "w",
                "a",
                "s",
                "d",
                "arrowup",
                "arrowdown",
                "arrowleft",
                "arrowright",
                "q",
                "e",
            ].includes(k)
        )
            return;
        e.preventDefault();
        if (k === "q") {
            yaw = clamp(yaw - YAW_STEP, -180, 180);
            sendManual();
            return;
        }
        if (k === "e") {
            yaw = clamp(yaw + YAW_STEP, -180, 180);
            sendManual();
            return;
        }
        if (!held.has(k)) {
            held.add(k);
            recomputeSticks();
            sendManual(); // immediate response on press
        }
    }
    function onKeyUp(e: KeyboardEvent) {
        if (mode !== "manual") return;
        const k = e.key.toLowerCase();
        if (held.delete(k)) {
            recomputeSticks();
            sendManual(); // send the release so sticks recenter on the STM
        }
    }

    // on-screen press/release mirror the keyboard (pointer + touch)
    function pressDir(dir: "fwd" | "back" | "left" | "right") {
        const map = { fwd: "w", back: "s", left: "a", right: "d" } as const;
        held.add(map[dir]);
        recomputeSticks();
        sendManual();
    }
    function releaseDir(dir: "fwd" | "back" | "left" | "right") {
        const map = { fwd: "w", back: "s", left: "a", right: "d" } as const;
        if (held.delete(map[dir])) {
            recomputeSticks();
            sendManual();
        }
    }

    // ---------- SEMI-AUTONOMOUS ----------
    function goto(dx: number, dy: number, dz: number) {
        // ENU metres: x=east, y=north, z=up. Published to drone/goto (one-shot).
        publish(TOPICS.GOTO, { x: dx, y: dy, z: dz });
    }
    function gotoDir(dir: "fwd" | "back" | "left" | "right") {
        if (dir === "fwd") goto(0, stepMeters, 0);
        if (dir === "back") goto(0, -stepMeters, 0);
        if (dir === "left") goto(-stepMeters, 0, 0);
        if (dir === "right") goto(stepMeters, 0, 0);
    }
    function gotoAltitude() {
        if (altDelta !== 0) goto(0, 0, altDelta);
    }

    // ---------- shared controls ----------
    function toggleArm() {
        armed = !armed;
        // arm state belongs to both modes; send on the control topic
        sendManual();
    }
    function onThrottle(e: Event) {
        throttle = clamp(Number((e.target as HTMLInputElement).value), 0, 1);
        if (mode === "manual") sendManual();
    }
    function setMode(m: Mode) {
        if (m === mode) return;
        if (mode === "manual") stopManualStream();
        mode = m;
        if (mode === "manual") startManualStream();
    }

    onMount(() => {
        window.addEventListener("keydown", onKeyDown);
        window.addEventListener("keyup", onKeyUp);
        if (mode === "manual") startManualStream();
    });
    onDestroy(() => {
        window.removeEventListener("keydown", onKeyDown);
        window.removeEventListener("keyup", onKeyUp);
        stopManualStream();
    });

    // live motor preview (manual mixing, same as flight controller)
    let motors = $derived.by(() => {
        if (!armed) return [0, 0, 0, 0] as const;
        const base = 1000 + throttle * 1000;
        const r = clamp(rollCmd / 45, -1, 1) * 80;
        const p = clamp(pitchCmd / 45, -1, 1) * 80;
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
    <div class="card__head card__head--row">
        <div>
            <h3 class="card__title">UAV controls</h3>
            <p class="card__desc">
                {mode === "manual"
                    ? "Hold W/A/S/D or arrow keys to fly. Q/E yaw."
                    : "Send a position delta; the drone navigates there and hovers."}
            </p>
        </div>
        <div class="mode-switch" role="tablist" aria-label="Control mode">
            <button
                type="button"
                role="tab"
                class="mode-btn"
                class:mode-btn--active={mode === "manual"}
                aria-selected={mode === "manual"}
                onclick={() => setMode("manual")}>Manual</button
            >
            <button
                type="button"
                role="tab"
                class="mode-btn"
                class:mode-btn--active={mode === "auto"}
                aria-selected={mode === "auto"}
                onclick={() => setMode("auto")}>Semi-auto</button
            >
        </div>
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
            <span class="field__label"
                >{mode === "manual" ? "Throttle" : "Hover throttle"}</span
            >
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

        {#if mode === "manual"}
            <div class="field-group">
                <p class="field-group__title">Attitude (hold to deflect)</p>
                <div class="manual-pad" aria-label="Manual flight pad">
                    <button
                        type="button"
                        class="manual-btn manual-btn--up"
                        class:manual-btn--active={pitchCmd > 0}
                        aria-label="Forward (W)"
                        onpointerdown={() => pressDir("fwd")}
                        onpointerup={() => releaseDir("fwd")}
                        onpointerleave={() => releaseDir("fwd")}>W</button
                    >
                    <button
                        type="button"
                        class="manual-btn manual-btn--left"
                        class:manual-btn--active={rollCmd < 0}
                        aria-label="Left (A)"
                        onpointerdown={() => pressDir("left")}
                        onpointerup={() => releaseDir("left")}
                        onpointerleave={() => releaseDir("left")}>A</button
                    >
                    <button
                        type="button"
                        class="manual-btn manual-btn--right"
                        class:manual-btn--active={rollCmd > 0}
                        aria-label="Right (D)"
                        onpointerdown={() => pressDir("right")}
                        onpointerup={() => releaseDir("right")}
                        onpointerleave={() => releaseDir("right")}>D</button
                    >
                    <button
                        type="button"
                        class="manual-btn manual-btn--down"
                        class:manual-btn--active={pitchCmd < 0}
                        aria-label="Backward (S)"
                        onpointerdown={() => pressDir("back")}
                        onpointerup={() => releaseDir("back")}
                        onpointerleave={() => releaseDir("back")}>S</button
                    >
                </div>
                <div class="yaw-row">
                    <span class="field__label">Yaw</span>
                    <output class="mono">{yaw.toFixed(0)}°</output>
                    <span class="hint">Q / E</span>
                </div>
            </div>
        {:else}
            <div class="field-group">
                <p class="field-group__title">Move (step distance)</p>
                <div class="range-row">
                    <input
                        type="range"
                        min="0.5"
                        max="10"
                        step="0.5"
                        bind:value={stepMeters}
                        class="range"
                    />
                    <output class="mono">{stepMeters.toFixed(1)} m</output>
                </div>
                <div class="manual-pad" aria-label="Go-to pad">
                    <button
                        type="button"
                        class="manual-btn manual-btn--up"
                        aria-label="North"
                        onclick={() => gotoDir("fwd")}>↑</button
                    >
                    <button
                        type="button"
                        class="manual-btn manual-btn--left"
                        aria-label="West"
                        onclick={() => gotoDir("left")}>←</button
                    >
                    <button
                        type="button"
                        class="manual-btn manual-btn--right"
                        aria-label="East"
                        onclick={() => gotoDir("right")}>→</button
                    >
                    <button
                        type="button"
                        class="manual-btn manual-btn--down"
                        aria-label="South"
                        onclick={() => gotoDir("back")}>↓</button
                    >
                </div>
                <div class="field-row">
                    <label class="field">
                        <span class="field__label">Altitude delta (m)</span>
                        <input
                            bind:value={altDelta}
                            type="number"
                            step="0.5"
                            class="field__input mono"
                        />
                    </label>
                    <button
                        type="button"
                        class="btn btn--primary"
                        onclick={gotoAltitude}
                    >
                        Apply altitude
                    </button>
                </div>
            </div>
        {/if}
    </div>
</article>

<style>
    .mode-switch {
        display: inline-flex;
        background: #eef2f6;
        border-radius: 999px;
        padding: 0.2rem;
        gap: 0.2rem;
    }
    .mode-btn {
        border: none;
        background: transparent;
        font: inherit;
        font-size: 0.82rem;
        font-weight: 600;
        padding: 0.35rem 0.8rem;
        border-radius: 999px;
        cursor: pointer;
        color: var(--text-muted);
    }
    .mode-btn--active {
        background: var(--surface);
        color: var(--accent);
        box-shadow: var(--shadow-sm);
    }
    .manual-btn--active {
        border-color: var(--accent);
        background: var(--accent-soft);
        transform: translateY(-1px);
    }
    .yaw-row {
        display: flex;
        align-items: center;
        gap: 0.6rem;
        margin-top: 0.6rem;
    }
    .yaw-row .hint,
    .hint {
        font-size: 0.72rem;
        color: var(--text-muted);
        margin-left: auto;
    }
</style>
