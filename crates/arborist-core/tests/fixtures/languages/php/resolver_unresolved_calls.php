<?php
function compute(int $value): int {
    return $value + 1;
}

function caller(int $value): int {
    return missing($value);
}
