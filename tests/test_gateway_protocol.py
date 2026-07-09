from __future__ import annotations

import importlib
import unittest

from tests.gateway_protocol import GROUP_SUITES, SUITE_MODULES


def load_tests(loader: unittest.TestLoader, _: unittest.TestSuite, __: str | None) -> unittest.TestSuite:
    suite = unittest.TestSuite()
    for suite_name in GROUP_SUITES["gateway"]:
        module_name = SUITE_MODULES[suite_name]
        suite.addTests(loader.loadTestsFromModule(importlib.import_module(module_name)))
    return suite


if __name__ == "__main__":
    unittest.main()
