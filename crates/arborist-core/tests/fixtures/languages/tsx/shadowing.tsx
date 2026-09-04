export function compute(value: number): number {
    const helper = (x: number): number => x * 2;
    return helper(value);
}
