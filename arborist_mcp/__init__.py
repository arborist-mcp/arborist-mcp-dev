from __future__ import annotations

from pathlib import Path

__version__ = "0.1.0"
__all__ = ["__version__"]

_SOURCE_PACKAGE_DIR = (
    Path(__file__).resolve().parent.parent / "python" / "arborist_mcp"
)

if _SOURCE_PACKAGE_DIR.is_dir():
    __path__ = [str(_SOURCE_PACKAGE_DIR)]
else:
    raise ModuleNotFoundError(
        f"Arborist source package not found at {_SOURCE_PACKAGE_DIR}"
    )
