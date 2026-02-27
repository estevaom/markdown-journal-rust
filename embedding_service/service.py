from __future__ import annotations

import os
from typing import List

import torch
from fastapi import FastAPI, HTTPException
from pydantic import BaseModel
from sentence_transformers import SentenceTransformer

app = FastAPI(title="Embedding Service", version="0.1.0")

# Global model (loaded once on startup)
model: SentenceTransformer | None = None
model_device: str | None = None
model_name: str | None = None


class EmbedRequest(BaseModel):
    texts: List[str]


class EmbedResponse(BaseModel):
    embeddings: List[List[float]]
    dimension: int
    model_name: str


def pick_device() -> str:
    override = os.environ.get("EMBEDDING_DEVICE") or os.environ.get("DEVICE")
    if override:
        return override

    if torch.cuda.is_available():
        return "cuda"

    if hasattr(torch.backends, "mps") and torch.backends.mps.is_available():
        return "mps"

    return "cpu"


@app.on_event("startup")
async def load_model() -> None:
    global model, model_device, model_name

    model_name = os.environ.get("EMBEDDING_MODEL") or "BAAI/bge-large-en-v1.5"
    model_device = pick_device()

    print(f"🚀 Loading {model_name} on {model_device}...")
    print("  This may take a while on first run (model download).")

    try:
        model = SentenceTransformer(model_name, device=model_device)
        print("✅ Model loaded successfully!")
        print(f"  Device: {model.device}")
        print(f"  Max sequence length: {model.max_seq_length}")
        print(f"  Embedding dimension: {model.get_sentence_embedding_dimension()}")
    except Exception as e:
        print(f"❌ Failed to load model: {e}")
        if model_device == "cuda":
            print(
                "  CUDA selected but not available. Install a CUDA-enabled PyTorch wheel, or set EMBEDDING_DEVICE=cpu."
            )
        raise


@app.post("/embed", response_model=EmbedResponse)
async def embed(request: EmbedRequest) -> EmbedResponse:
    if model is None:
        raise HTTPException(status_code=503, detail="Model not loaded")

    if not request.texts:
        raise HTTPException(status_code=400, detail="Empty texts list")

    try:
        embeddings = model.encode(
            request.texts,
            batch_size=32,
            show_progress_bar=False,
            convert_to_numpy=True,
            normalize_embeddings=True,  # Important for cosine similarity via inner product
        )

        return EmbedResponse(
            embeddings=embeddings.tolist(),
            dimension=int(embeddings.shape[1]),
            model_name=model_name or "unknown",
        )
    except Exception as e:
        raise HTTPException(status_code=500, detail=f"Embedding generation failed: {str(e)}")


@app.get("/health")
async def health() -> dict:
    return {
        "status": "healthy" if model is not None else "model_not_loaded",
        "model_loaded": model is not None,
        "device": str(model.device) if model else None,
        "device_selected": model_device,
        "cuda_available": torch.cuda.is_available(),
        "cuda_device_count": torch.cuda.device_count() if torch.cuda.is_available() else 0,
        "mps_available": hasattr(torch.backends, "mps") and torch.backends.mps.is_available(),
        "mps_built": hasattr(torch.backends, "mps") and torch.backends.mps.is_built(),
    }


@app.get("/info")
async def info() -> dict:
    if model is None:
        raise HTTPException(status_code=503, detail="Model not loaded")

    return {
        "model_name": model_name or "unknown",
        "dimensions": model.get_sentence_embedding_dimension(),
        "device": str(model.device),
        "max_seq_length": model.max_seq_length,
        "cuda_version": torch.version.cuda if torch.cuda.is_available() else None,
    }


@app.get("/")
async def root() -> dict:
    return {
        "service": "Embedding Service",
        "version": "0.1.0",
        "model": model_name or "unknown",
        "endpoints": {
            "/embed": "POST - Generate embeddings",
            "/health": "GET - Health check",
            "/info": "GET - Model information",
        },
    }

