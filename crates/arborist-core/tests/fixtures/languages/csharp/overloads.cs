namespace Demo {
    public static class Overloads {
        static int helper(int value) => value + 1;
        static int helper(string value) => value.Length;
        public static int orchestrate(int value) => helper(value);
    }
}
