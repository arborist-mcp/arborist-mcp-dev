from __future__ import annotations

from importlib import import_module
from pathlib import Path
from pkgutil import extend_path

_SOURCE_PACKAGE_DIR = (
    Path(__file__).resolve().parent.parent / "python" / "arborist_mcp"
)

if _SOURCE_PACKAGE_DIR.is_dir():
    source_package_dir = str(_SOURCE_PACKAGE_DIR)
    __path__ = extend_path(__path__, __name__)
    if source_package_dir not in __path__:
        __path__.insert(0, source_package_dir)
else:
    raise ModuleNotFoundError(
        f"Arborist source package not found at {_SOURCE_PACKAGE_DIR}"
    )

__version__ = import_module(f"{__name__}._version").__version__

__all__ = ["__version__"]
