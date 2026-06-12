<script lang="ts">
    import { onMount, onDestroy } from "svelte";
    import { publish, TOPICS } from "../lib/mqtt";
    import { clamp } from "../lib/utils";
    import type { ControlState } from "../lib/types";

    // shared
    let armed = $state(false);
    let throttle = $state(0);
    let yaw = $state(0);
    let tick = $state(0);

    // deflection magnitude applied while a key is held, in degrees of desired tilt
    const TILT = 12; // max commanded roll/pitch angle
    const YAW_STEP = 30; // yaw nudge per keypress (deg)
    const SEND_HZ = 50; // manual stream rate — matches STM RC_HZ for the failsafe

    let pitchCmd = $state(0); // forward/back  (W/S or Up/Down)
    let rollCmd = $state(0); // left/right    (A/D or Left/Right)
    const held = new Set<string>();
    let streamTimer: ReturnType<typeof setInterval> | null = null;

    function bumpTick() {
        tick = (tick + 1) >>> 0;
    }

    function recomputeSticks() {
        const fwd = held.has("w") || held.has("arrowup");
        const back = held.has("s") || held.has("arrowdown");
        const left = held.has("a") || held.has("arrowleft");
        const right = held.has("d") || held.has("arrowright");
        pitchCmd = (fwd ? TILT : 0) - (back ? TILT : 0);
        rollCmd = (right ? TILT : 0) - (left ? TILT : 0);
    }

    function send() {
        bumpTick();
        const state: ControlState = {
            armed,
            throttle,
            orientation: { roll: rollCmd, pitch: pitchCmd, yaw },
            tick,
        };
        publish(TOPICS.CONTROL, state);
    }

    // Stream at a fixed rate so the STM's link-loss watchdog stays fed even when
    // sticks are centered and no key events are firing.
    function startStream() {
        if (streamTimer) return;
        streamTimer = setInterval(send, 1000 / SEND_HZ);
    }
    function stopStream() {
        if (streamTimer) {
            clearInterval(streamTimer);
            streamTimer = null;
        }
        pitchCmd = 0;
        rollCmd = 0;
        held.clear();
    }

    function onKeyDown(e: KeyboardEvent) {
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
            return;
        }
        if (k === "e") {
            yaw = clamp(yaw + YAW_STEP, -180, 180);
            return;
        }
        if (!held.has(k)) {
            held.add(k);
            recomputeSticks();
        }
    }
    function onKeyUp(e: KeyboardEvent) {
        const k = e.key.toLowerCase();
        if (held.delete(k)) {
            recomputeSticks();
        }
    }

    function pressDir(dir: "fwd" | "back" | "left" | "right") {
        const map = { fwd: "w", back: "s", left: "a", right: "d" } as const;
        held.add(map[dir]);
        recomputeSticks();
    }
    function releaseDir(dir: "fwd" | "back" | "left" | "right") {
        const map = { fwd: "w", back: "s", left: "a", right: "d" } as const;
        if (held.delete(map[dir])) {
            recomputeSticks();
        }
    }

    function toggleArm() {
        armed = !armed;
        send();
    }
    function onThrottle(e: Event) {
        throttle = clamp(Number((e.target as HTMLInputElement).value), 0, 1);
    }

    onMount(() => {
        window.addEventListener("keydown", onKeyDown);
        window.addEventListener("keyup", onKeyUp);
        startStream();
    });
    onDestroy(() => {
        window.removeEventListener("keydown", onKeyDown);
        window.removeEventListener("keyup", onKeyUp);
        stopStream();
    });

    // live motor preview — mirrors the flight controller's mix and sign rules.
    // Right motors get -roll, rear motors get -pitch (see motors.rs::mix).
    let motors = $derived.by(() => {
        if (!armed) return [0, 0, 0, 0] as const;
        const base = 1000 + throttle * 1000;
        const r = clamp(rollCmd / 45, -1, 1) * 80;
        const p = clamp(pitchCmd / 45, -1, 1) * 80;
        const y = clamp(yaw / 90, -1, 1) * 60;
        return [
            Math.round(clamp(base + r + p + y, 1000, 2000)), // FL
            Math.round(clamp(base - r + p - y, 1000, 2000)), // FR
            Math.round(clamp(base + r - p - y, 1000, 2000)), // RL
            Math.round(clamp(base - r - p + y, 1000, 2000)), // RR
        ] as const;
    });
</script>

<article class="card card--controls">
    <div class="card__head card__head--row">
        <div>
            <h3 class="card__title">UAV controls</h3>
            <p class="card__desc">
                Hold W/A/S/D or arrow keys to fly. Q/E yaw.
            </p>
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
            <span class="field__label">Throttle</span>
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
    </div>
</article>

<style>
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
