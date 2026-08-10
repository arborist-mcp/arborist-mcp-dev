package demo;

public final class Demo {
    static int helper(int value) { return value + 1; }

    public static int orchestrate(int value) { return helper(value); }
}
