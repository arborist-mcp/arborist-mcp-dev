from __future__ import annotations

from .gateway_symbol_list_routes import GatewaySymbolListRoutes
from .gateway_symbol_read_routes import GatewaySymbolReadRoutes
from .gateway_symbol_search_routes import GatewaySymbolSearchRoutes


class GatewaySymbolRoutes(
    GatewaySymbolReadRoutes,
    GatewaySymbolSearchRoutes,
    GatewaySymbolListRoutes,
):
    """Symbol read/search/list route handlers for the MCP gateway."""


