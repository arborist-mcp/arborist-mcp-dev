package demo

func helper(value int) int { return value + 1 }

func Orchestrate(value int) int { return helper(value) }
