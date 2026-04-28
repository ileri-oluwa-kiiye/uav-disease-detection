export function isHealthyLabel(prediction: string | null | undefined): boolean {
    if (!prediction) return false;
    return String(prediction).toLowerCase().includes("healthy");
}

export function formatTs(iso: string): string {
    try {
        return new Date(iso).toLocaleString();
    } catch {
        return iso;
    }
}

export function clamp(value: number, min: number, max: number): number {
    return Math.min(max, Math.max(min, value));
}

export function isDataImageUrl(u: unknown): u is string {
    return (
        typeof u === "string" &&
        /^data:image\/(jpeg|jpg|png|webp);base64,/i.test(u)
    );
}
