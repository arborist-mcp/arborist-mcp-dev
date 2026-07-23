from __future__ import annotations

from .gateway_patch_apply_routes import GatewayPatchApplyRoutes
from .gateway_patch_validation_routes import GatewayPatchValidationRoutes


class GatewayPatchRoutes(GatewayPatchApplyRoutes, GatewayPatchValidationRoutes):
    """Patch apply/preview and validation route handlers for the MCP gateway."""

