use anyhow::{Result, bail};
use tree_sitter::Point;

use crate::model::Position;

pub fn position_from(point: Point) -> Position {
    Position {
        row: point.row,
        column: point.column,
    }
}

pub fn point_for_offset(source: &str, byte_offset: usize) -> Result<Point> {
    if byte_offset > source.len() {
        bail!(
            "byte offset {} is out of bounds for source of length {}",
            byte_offset,
            source.len()
        );
    }
    if !source.is_char_boundary(byte_offset) {
        bail!(
            "byte offset {} does not align to a UTF-8 character boundary",
            byte_offset
        );
    }

    let mut row = 0;
    let mut column = 0;
    for byte in source.as_bytes().iter().take(byte_offset) {
        if *byte == b'\n' {
            row += 1;
            column = 0;
        } else {
            column += 1;
        }
    }

    Ok(Point { row, column })
}

pub fn offset_for_position(source: &str, position: &Position) -> Result<usize> {
    let mut row = 0;
    let mut column = 0;

    for (index, byte) in source.as_bytes().iter().enumerate() {
        if row == position.row && column == position.column {
            if !source.is_char_boundary(index) {
                bail!(
                    "position {}:{} maps to byte offset {} which does not align to a UTF-8 character boundary",
                    position.row,
                    position.column,
                    index
                );
            }
            return Ok(index);
        }

        if *byte == b'\n' {
            row += 1;
            column = 0;
        } else {
            column += 1;
        }
    }

    if row == position.row && column == position.column {
        return Ok(source.len());
    }

    bail!(
        "position {}:{} is out of bounds for source",
        position.row,
        position.column
    )
}
