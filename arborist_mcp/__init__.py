from __future__ import annotations

from importlib import import_module
from pathlib import Path

_SOURCE_PACKAGE_DIR = (
    Path(__file__).resolve().parent.parent / "python" / "arborist_mcp"
)

if _SOURCE_PACKAGE_DIR.is_dir():
    __path__ = [str(_SOURCE_PACKAGE_DIR)]
else:
    raise ModuleNotFoundError(
        f"Arborist source package not found at {_SOURCE_PACKAGE_DIR}"
    )

__version__ = import_module(f"{__name__}._version").__version__

__all__ = ["__version__"]
