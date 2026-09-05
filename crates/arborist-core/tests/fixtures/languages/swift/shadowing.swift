func compute(value: Int) -> Int {
    return value + 1;
}

func caller(value: Int) -> Int {
    func helper(value: Int) -> Int {
        return value * 2;
    }
    return helper(value);
}
