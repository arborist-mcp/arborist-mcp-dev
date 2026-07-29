from __future__ import annotations

from tests.gateway_protocol.helpers import GatewayProtocolTestCase
from tests.gateway_protocol.runtime_catalog import (
    GatewayRuntimeCatalogTestsMixin,
)
from tests.gateway_protocol.runtime_transport import (
    GatewayRuntimeTransportTestsMixin,
)
from tests.gateway_protocol.runtime_tools import GatewayRuntimeToolsTestsMixin
from tests.gateway_protocol.runtime_tree_query import (
    GatewayRuntimeTreeQueryTestsMixin,
)

SUITE_NAME = "gateway-runtime"
REQUIRES_EXTENSION = True
COVERED_TOOLS = (
    "arborist/batch",
    "arborist/execute_tree_query",
    "arborist/get_semantic_skeleton",
    "arborist/list_symbol_indexes",
)


class GatewayRuntimeTests(
    GatewayRuntimeCatalogTestsMixin,
    GatewayRuntimeToolsTestsMixin,
    GatewayRuntimeTransportTestsMixin,
    GatewayRuntimeTreeQueryTestsMixin,
    GatewayProtocolTestCase,
):
    pass
