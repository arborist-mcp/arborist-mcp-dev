int helper(int value) { return value + 1; }
int helper(double value) { return static_cast<int>(value) + 2; }

int orchestrate(int value) { return helper(value); }
