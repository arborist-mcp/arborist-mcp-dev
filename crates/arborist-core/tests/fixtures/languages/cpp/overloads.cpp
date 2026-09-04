int helper(int value) { return value + 1; }
int helper(const char* value) { return value == nullptr ? 0 : 1; }

int orchestrate(int value) { return helper(value); }
