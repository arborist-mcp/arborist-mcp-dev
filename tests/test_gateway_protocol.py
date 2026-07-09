from __future__ import annotations

import importlib
import unittest

from tests import GROUP_MODULES


def load_tests(loader: unittest.TestLoader, _: unittest.TestSuite, __: str | None) -> unittest.TestSuite:
    suite = unittest.TestSuite()
    for module_name in GROUP_MODULES["gateway"]:
        suite.addTests(loader.loadTestsFromModule(importlib.import_module(module_name)))
    return suite


if __name__ == "__main__":
    unittest.main()
