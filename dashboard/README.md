# AgroAI Monitor — static dashboard

Vanilla HTML/CSS/JS UI for the UAV tomato disease FastAPI service.

## Prerequisites

- Python environment with FastAPI dependencies installed (see `../fastapi/requirements.txt`).
- Model file `tomato_model.pth` in the FastAPI project directory (as used by `app/model.py`).

## 1. Start the API

From the `fastapi` folder (with your virtualenv activated if you use one):

```bash
cd fastapi
uvicorn app.main:app --reload --host 127.0.0.1 --port 8000
```

The API exposes:

- `POST /predict` — form fields: `file` (image), `latitude`, `longitude`
- `GET /predictions` — JSON list of stored predictions (in-memory; resets when the server restarts)

## 2. Serve the dashboard

Browsers block ES modules and often block `fetch` to another origin when opening `index.html` as a `file://` URL. Serve this folder over HTTP, for example:

```bash
cd dashboard
python3 -m http.server 5500
```

Open **http://127.0.0.1:5500** in your browser.

## 3. API base URL

If the backend is not on `http://127.0.0.1:8000`, edit `API_BASE` in `js/api.js`.

## Notes

- CORS is enabled on the FastAPI app for local development.
- Prediction thumbnails are stored as JPEG data URLs in memory for the map popups and history table.
