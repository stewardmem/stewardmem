#!/usr/bin/env python3
"""glm-ocr-mlx.py — the vlm tier's ENGINE (IMPROVE16.1, S2).

The model-serving half of `scripts/glm-ocr-kernel.sh` (the docling-kernel
precedent: committed tooling, operator-run in their own venv — it needs the
mlx-vlm package, so it is run as
`<venv>/bin/python scripts/glm-ocr-mlx.py …`, never `python3 -I`).
Dependencies: mlx, mlx-vlm only.

    glm-ocr-mlx.py <out-dir> <image.png> [<image.png> ...]

For the Nth image (in argv order) writes:

  <out-dir>/page-NNN.txt  the model's output AS PRODUCED — this engine never
                          rewrites, smooths, or post-corrects model bytes
                          (arXiv:2610.30026's cue-loss class is measured,
                          never fixed; the correction is the OTHER lane's
                          governed job)
  <out-dir>/stamp.txt     one line, the tool-tier stamp law's by-record
                          identity: `<model-id>@mlx-vlm-<version>` (the quant
                          is part of the model id). The SHIM turns this into
                          meta.json's `version` — recorded per run, never
                          lockfile-pinned.

Laws this file exists to keep:

- **Declared, not guaranteed** (DI-1 does not hold for a VLM tier,
  preregistered): temperature 0.0 is DECLARED here and stamped per run;
  reproducible-BY-RECORD, never byte-reproducible.
- **The canonical plain-extraction prompt**: `"Text Recognition:"` — the
  upstream card's fixed prompt for plain text extraction, pinned by
  constant. No other prompt surface exists in this file.
- **Offline by posture**: the SHIM pins HF_HUB_OFFLINE=1 /
  TRANSFORMERS_OFFLINE=1 before invoking this engine — the model loads from
  the operator's pre-fetched HF cache (`huggingface-cli download
  <model-id>`), and nothing here speaks to a remote endpoint. (The load may
  still answer a local cache-index read; that is the artifacts law's
  posture, no fetch, no data leaves.)
"""

import pathlib
import sys
import time

import mlx.core as mx  # noqa: F401 — imported for the stamp's environment
import mlx_vlm
from mlx_vlm import generate, load
from mlx_vlm.prompt_utils import apply_chat_template

MODEL_ID = "mlx-community/GLM-OCR-4bit"
PROMPT = "Text Recognition:"  # the upstream card's canonical plain-extraction prompt
MAX_TOKENS = 4096  # a dense A4 page's markdown — the round's budget constant
TEMPERATURE = 0.0  # DECLARED, not guaranteed (DI-1 does not hold for a VLM tier)


def fail(msg: str) -> None:
    print(f"glm-ocr-mlx: {msg}", file=sys.stderr)
    sys.exit(1)


def main() -> None:
    if len(sys.argv) < 3:
        fail("usage: glm-ocr-mlx.py <out-dir> <image.png> [<image.png> ...]")
    out_dir = pathlib.Path(sys.argv[1])
    images = [pathlib.Path(p) for p in sys.argv[2:]]
    for img in images:
        if not img.is_file():
            fail(f"input is not a file: {img}")
    out_dir.mkdir(parents=True, exist_ok=True)

    model_id = MODEL_ID
    try:
        t0 = time.time()
        model, processor = load(model_id)
        load_secs = round(time.time() - t0, 1)
    except Exception as e:  # noqa: BLE001 — the shim bounds whatever this prints
        fail(f"the model could not be loaded from the local cache "
             f"(pre-fetch it: huggingface-cli download {model_id}): {e}")

    for i, img in enumerate(images):
        try:
            formatted = apply_chat_template(
                processor, model.config, PROMPT, num_images=1
            )
            result = generate(
                model,
                processor,
                formatted,
                image=[str(img)],
                max_tokens=MAX_TOKENS,
                temperature=TEMPERATURE,
                verbose=False,
            )
        except Exception as e:  # noqa: BLE001 — the shim bounds whatever this prints
            fail(f"generation failed for {img.name}: {e}")
        (out_dir / f"page-{i:03d}.txt").write_text(result.text)
        print(
            f"page-{i:03d}: {result.generation_tokens} tokens, "
            f"finish={result.finish_reason}",
            file=sys.stderr,
        )

    stamp = f"{model_id}@mlx-vlm-{mlx_vlm.__version__}"
    (out_dir / "stamp.txt").write_text(stamp + "\n")
    # stderr progress (the kernel silences stdout and bounds stderr):
    print(
        f"glm-ocr-mlx: {len(images)} pages in {round(time.time() - t0, 1)}s "
        f"(load {load_secs}s), model {model_id}, mlx {mx.__version__}, "
        f"mlx-vlm {mlx_vlm.__version__}",
        file=sys.stderr,
    )


if __name__ == "__main__":
    main()
