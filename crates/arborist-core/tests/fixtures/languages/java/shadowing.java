package demo;

public final class Shadowing {
    static int compute(int value) { return value + 1; }

    public static int orchestrate(int value) {
        int compute = value * 2;
        return compute + value;
    }
}
