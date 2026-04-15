# app/inference.py

import torch
from PIL import Image
from torchvision import transforms
from app.utils import format_prediction

# SAME preprocessing as validation (VERY IMPORTANT)
val_transforms = transforms.Compose([
    transforms.Resize((300, 300)),
    transforms.ToTensor(),
    transforms.Normalize(
        mean=[0.485, 0.456, 0.406],
        std =[0.229, 0.224, 0.225]
    )
])


def predict_image(image: Image.Image, model, class_names):
    """
    Takes a PIL image and returns predicted class + confidence
    """

    # Preprocess
    image = image.convert("RGB")
    image = val_transforms(image)
    image = image.unsqueeze(0)  # shape: [1, 3, H, W]

    # Inference
    with torch.no_grad():
        outputs = model(image)
        probs = torch.softmax(outputs, dim=1)
        confidence, pred_idx = torch.max(probs, dim=1)

    predicted_class = class_names[pred_idx.item()]
    confidence = confidence.item()

    return format_prediction(predicted_class, confidence)