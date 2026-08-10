#include <stddef.h>

static int helper(int value) { return value + 1; }

int orchestrate(int value) { return helper(value); }
