# Direct calls and imports (trace scenario).
import helper_module

def compute(value: int) -> int:
    return value + 1

def orchestrate(value: int) -> int:
    local = compute(value)
    return helper_module.helper(local)
