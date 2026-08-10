package demo

func helper(value int) int { return value + 1 }

func Orchestrate(value int) int {
    helper := func(x int) int { return x * 2 }
    return helper(value)
}
