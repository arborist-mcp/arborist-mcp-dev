package demo

fun compute(value: Int): Int = value + 1

fun orchestrate(value: Int): Int {
    val compute = value * 2
    return compute + value
}
