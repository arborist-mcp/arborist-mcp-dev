package demo

fun helper(value: Int): Int = value + 1
fun helper(value: String): Int = value.length

fun orchestrate(value: Int): Int = helper(value)
