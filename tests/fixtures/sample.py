class Greeter:
    def greet(self, name: str) -> str:
        return f"hello, {name}"


def top_level(value: int) -> int:
    def nested(inner: int) -> int:
        return inner + 1

    return nested(value)

