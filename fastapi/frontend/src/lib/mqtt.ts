import mqtt, { type MqttClient } from "mqtt";
import { writable } from "svelte/store";
import type { TelemetryData, ConnectionStatus } from "./types";

export const connectionStatus = writable<ConnectionStatus>("disconnected");

export const telemetry = writable<TelemetryData>({
    attitude: { roll: 0, pitch: 0, yaw: 0 },
    motors: [0, 0, 0, 0],
    armed: false,
    tick: 0,
});

export const TOPICS = {
    CONTROL: "drone/control",
    TELEMETRY: "drone/telemetry",
} as const;

let client: MqttClient | null = null;

export interface MqttConfig {
    brokerUrl: string;
    username?: string;
    password?: string;
}

export function connect(config: MqttConfig): MqttClient {
    if (client) return client;

    connectionStatus.set("connecting");

    client = mqtt.connect(config.brokerUrl, {
        username: config.username,
        password: config.password,
        reconnectPeriod: 3000,
        connectTimeout: 10_000,
    });

    client.on("connect", () => {
        connectionStatus.set("connected");
        client!.subscribe(TOPICS.TELEMETRY, { qos: 0 });
    });

    client.on("close", () => {
        connectionStatus.set("disconnected");
    });

    client.on("reconnect", () => {
        connectionStatus.set("connecting");
    });

    client.on("message", (topic, payload) => {
        if (topic === TOPICS.TELEMETRY) {
            try {
                const data = JSON.parse(payload.toString()) as TelemetryData;
                telemetry.set(data);
            } catch {
                console.warn("Bad telemetry payload:", payload.toString());
            }
        }
    });

    return client;
}

export function publish(topic: string, payload: unknown): void {
    if (!client || !client.connected) {
        console.warn("MQTT not connected, dropping publish to", topic);
        return;
    }
    const msg = typeof payload === "string" ? payload : JSON.stringify(payload);
    client.publish(topic, msg, { qos: 0 });
}

export function disconnect(): void {
    if (client) {
        client.end(true);
        client = null;
        connectionStatus.set("disconnected");
    }
}
