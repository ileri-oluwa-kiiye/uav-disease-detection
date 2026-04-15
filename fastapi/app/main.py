# app/main.py

import base64
import io
import uuid
from datetime import datetime, timezone
from typing import List

from fastapi import FastAPI, File, Form, UploadFile
from fastapi.middleware.cors import CORSMiddleware
from PIL import Image

from app.model import load_model
from app.inference import predict_image
from app.utils import read_imagefile

app = FastAPI(title="Tomato Disease Classifier")

app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"],
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)

# In-memory history for the dashboard (clears on server restart)
predictions_history: List[dict] = []

# Load model once at startup
model, class_names = load_model()


def _thumbnail_data_url(image_bytes: bytes, max_side: int = 640) -> str:
    img = Image.open(io.BytesIO(image_bytes)).convert("RGB")
    img.thumbnail((max_side, max_side))
    buf = io.BytesIO()
    img.save(buf, format="JPEG", quality=85)
    b64 = base64.b64encode(buf.getvalue()).decode("ascii")
    return f"data:image/jpeg;base64,{b64}"


@app.get("/")
def home():
    return {"message": "Tomato Disease API is running"}


@app.get("/predictions")
def get_predictions():
    return {"predictions": list(reversed(predictions_history))}


@app.post("/predict")
async def predict(
    file: UploadFile = File(...),
    latitude: float = Form(...),
    longitude: float = Form(...),
):
    image_bytes = await file.read()
    image = read_imagefile(image_bytes)

    result = predict_image(image, model, class_names)

    pred_id = str(uuid.uuid4())
    ts = datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")
    thumb_url = _thumbnail_data_url(image_bytes)

    record = {
        "id": pred_id,
        "prediction": result["prediction"],
        "confidence": result["confidence"],
        "latitude": latitude,
        "longitude": longitude,
        "timestamp": ts,
        "image_url": thumb_url,
    }
    predictions_history.append(record)

    return {
        **result,
        "id": pred_id,
        "latitude": latitude,
        "longitude": longitude,
        "timestamp": ts,
    }