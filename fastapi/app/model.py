# app/model.py

import torch
import torch.nn as nn
import torchvision.models as models


def load_model(model_path="tomato_model.pth"):
    NUM_CLASSES = 10

    # recreate architecture
    model = models.efficientnet_b3(weights=None)

    in_features = model.classifier[1].in_features

    model.classifier = nn.Sequential(
        nn.BatchNorm1d(in_features),
        nn.Dropout(p=0.5),
        nn.Linear(in_features, NUM_CLASSES)
    )

    # load weights
    checkpoint = torch.load(model_path, map_location="cpu")
    model.load_state_dict(checkpoint["model_state_dict"])

    class_names = checkpoint["class_names"]

    model.eval()

    return model, class_names