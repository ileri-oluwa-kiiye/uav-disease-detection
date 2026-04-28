<script lang="ts">
    import { telemetry } from "../lib/mqtt";

    let data = $derived($telemetry);
</script>

<article class="card card--telemetry">
    <div class="card__head">
        <h3 class="card__title">Telemetry</h3>
        <p class="card__desc">Live values from the ESP32-S3 via MQTT.</p>
    </div>

    <div class="telemetry-grid">
        <div class="telemetry-item">
            <span class="telemetry-item__label">Attitude</span>
            <span class="telemetry-item__value mono">
                Roll: {data.attitude.roll.toFixed(1)}, Pitch: {data.attitude.pitch.toFixed(
                    1,
                )}, Yaw: {data.attitude.yaw.toFixed(1)}
            </span>
        </div>
        <div class="telemetry-item">
            <span class="telemetry-item__label">Motor duties</span>
            <span class="telemetry-item__value mono">
                FL: {data.motors[0]}, FR: {data.motors[1]}, RL: {data
                    .motors[2]}, RR: {data.motors[3]}
            </span>
        </div>
        <div class="telemetry-item">
            <span class="telemetry-item__label">Armed?</span>
            <span class="telemetry-item__value mono"
                >{data.armed ? "Yes" : "No"}</span
            >
        </div>
        <div class="telemetry-item">
            <span class="telemetry-item__label">Tick</span>
            <span class="telemetry-item__value mono">{data.tick}</span>
        </div>
    </div>
</article>
