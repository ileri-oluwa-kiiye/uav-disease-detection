# app/utils.py

from PIL import Image
import io


def read_imagefile(file_bytes) -> Image.Image:
    """
    Converts uploaded file bytes to PIL Image
    """
    image = Image.open(io.BytesIO(file_bytes))
    return image.convert("RGB")


def format_prediction(prediction: str, confidence: float):
    """
    Standardize API output format
    """
    return {
        "prediction": prediction,
        "confidence": round(confidence, 4)
    }