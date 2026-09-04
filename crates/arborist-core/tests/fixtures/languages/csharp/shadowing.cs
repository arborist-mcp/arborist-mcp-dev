using System;

namespace Demo {
    public class Shadowing {
        static int Compute(int value) => value + 1;
        public static int Orchestrate(int value) {
            int compute = value * 2;
            return compute + value;
        }
    }
}
