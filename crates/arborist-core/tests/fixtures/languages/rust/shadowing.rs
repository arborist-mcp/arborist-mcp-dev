fn helper(value: i32) -> i32 { value + 1 }

pub fn orchestrate(value: i32) -> i32 {
    let helper = |x: i32| x * 2;
    helper(value)
}
