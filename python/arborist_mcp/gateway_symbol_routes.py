from __future__ import annotations

from typing import Any

from .gateway_symbol_list_routes import GatewaySymbolListRoutes
from .gateway_symbol_read_routes import GatewaySymbolReadRoutes
from .gateway_symbol_search_routes import GatewaySymbolSearchRoutes


class GatewaySymbolRoutes(
    GatewaySymbolReadRoutes,
    GatewaySymbolSearchRoutes,
    GatewaySymbolListRoutes,
):
    """Symbol read/search/list route handlers for the MCP gateway."""

    @staticmethod
    def _call_with_optional_timeout(
        method: Any,
        args: tuple[Any, ...],
        timeout_ms: int | None,
        *,
        omitted_before_timeout: tuple[Any, ...] = (),
    ) -> Any:
        if timeout_ms is None:
            return method(*args)
        return method(*args, *omitted_before_timeout, timeout_ms)


