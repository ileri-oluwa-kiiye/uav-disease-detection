# app/main.py

from fastapi import FastAPI, File, UploadFile
from PIL import Image
import io

from app.model import load_model
from app.inference import predict_image
from app.utils import read_imagefile

app = FastAPI(title="Tomato Disease Classifier")

# Load model once at startup
model, class_names = load_model()


@app.get("/")
def home():
    return {"message": "Tomato Disease API is running"}


@app.post("/predict")
async def predict(file: UploadFile = File(...)):
    # Read image
    image_bytes = await file.read()
    image = read_imagefile(image_bytes)

    # Predict
    result = predict_image(image, model, class_names)

    return result