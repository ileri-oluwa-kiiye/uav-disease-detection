import type { PredictResponse, PredictionsResponse } from "./types";

const API_BASE = import.meta.env.VITE_API_BASE ?? "";

async function handleResponse<T>(response: Response): Promise<T> {
    const text = await response.text();
    let data: any = null;
    try {
        data = text ? JSON.parse(text) : null;
    } catch {
        data = { detail: text || "Invalid response from server" };
    }
    if (!response.ok) {
        const msg =
            data?.detail ??
            data?.message ??
            `Request failed (${response.status})`;
        const err = new Error(
            typeof msg === "string" ? msg : JSON.stringify(msg),
        );
        (err as any).status = response.status;
        (err as any).data = data;
        throw err;
    }
    return data as T;
}

export async function postPredict(
    formData: FormData,
): Promise<PredictResponse> {
    const res = await fetch(`${API_BASE}/predict`, {
        method: "POST",
        body: formData,
    });
    return handleResponse<PredictResponse>(res);
}

export async function fetchPredictions(): Promise<PredictionsResponse> {
    const res = await fetch(`${API_BASE}/predictions`);
    return handleResponse<PredictionsResponse>(res);
}
