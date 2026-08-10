package demo;

public final class Overloads {
    static int helper(int value) { return value + 1; }
    static int helper(String value) { return value.length(); }

    public static int orchestrate(int value) { return helper(value); }
}
