export interface ControlState {
    armed: boolean;
    throttle: number;
    position: { x: number; y: number; z: number };
    orientation: { roll: number; pitch: number; yaw: number };
    tick: number;
}

export interface TelemetryData {
    attitude: { roll: number; pitch: number; yaw: number };
    motors: [number, number, number, number];
    armed: boolean;
    tick: number;
}

export interface Prediction {
    prediction: string;
    confidence: number;
    latitude: number;
    longitude: number;
    timestamp: string;
    image_url?: string;
}

export interface PredictResponse extends Prediction {}

export interface PredictionsResponse {
    predictions: Prediction[];
}

export type Tab = "dashboard" | "map" | "history";

export type ManualCommand = "forward" | "backward" | "left" | "right";

export type ConnectionStatus = "disconnected" | "connecting" | "connected";
