# Shadowing: the inner `compute` hides the outer one.
def compute(value: int) -> int:
    return value + 1

def orchestrate(value: int) -> int:
    def compute(x: int) -> int:
        return x * 2
    return compute(value)
