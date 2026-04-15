/**
 * FastAPI client — FormData for /predict, JSON for /predictions.
 * Change API_BASE if the backend runs elsewhere.
 */
const API_BASE = "http://127.0.0.1:8000";

async function handleResponse(response) {
  const text = await response.text();
  let data = null;
  try {
    data = text ? JSON.parse(text) : null;
  } catch {
    data = { detail: text || "Invalid response from server" };
  }
  if (!response.ok) {
    const msg =
      (data && (data.detail || data.message)) ||
      `Request failed (${response.status})`;
    const err = new Error(typeof msg === "string" ? msg : JSON.stringify(msg));
    err.status = response.status;
    err.data = data;
    throw err;
  }
  return data;
}

/**
 * @param {FormData} formData — must include: file, latitude, longitude
 */
export async function postPredict(formData) {
  const res = await fetch(`${API_BASE}/predict`, {
    method: "POST",
    body: formData,
  });
  return handleResponse(res);
}

export async function fetchPredictions() {
  const res = await fetch(`${API_BASE}/predictions`);
  return handleResponse(res);
}

export { API_BASE };
