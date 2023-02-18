//! Marching squares implementation
//!
//! We define the directions as below.
//! Note that screen coordinates often invert vertically, but marching squares algorithm
//! does not care. It just works numerically and you need to filp polarity accordingly.
//!
//! ```text
//!      ^ +y
//!  3 --+-- 2
//!  |   |   |
//! -+---+---+-> +x
//!  |   |   |
//!  0---+---1
//!      |
//! ```
//!

use crate::shape::Shape;
use std::collections::{HashMap, HashSet};

/// A trait for objects that can behave like a field of booleans.
pub trait AsBoolField {
    fn get(&self, pos: (isize, isize)) -> bool;
    fn shape(&self) -> Shape;

    fn get_or_false(&self, pos: Shape) -> bool {
        if pos.0 < 0 || pos.1 < 0 || self.shape().0 <= pos.0 || self.shape().1 <= pos.1 {
            false
        } else {
            self.get(pos)
        }
    }
}

/// A concrete type of a boolean field.
#[derive(Clone, Copy)]
pub struct BoolField<'a> {
    field: &'a [bool],
    shape: Shape,
}

impl<'a> BoolField<'a> {
    pub fn new(field: &'a [bool], shape: Shape) -> Self {
        Self { field, shape }
    }
}

impl<'a> AsBoolField for BoolField<'a> {
    fn get(&self, pos: (isize, isize)) -> bool {
        self.field[pos.0 as usize + pos.1 as usize * self.shape.0 as usize]
    }

    fn shape(&self) -> Shape {
        self.shape
    }
}

/// A concrete type of a boolean field.
/// Since we need a threshold value to compare, we have an extra field compared to [`BoolField`].
#[derive(Clone, Copy)]
pub(crate) struct F64Field<'a> {
    field: &'a [f64],
    shape: Shape,
    threshold: f64,
}

impl<'a> F64Field<'a> {
    #[allow(dead_code)]
    fn new(field: &'a [f64], shape: Shape, threshold: f64) -> Self {
        Self {
            field,
            shape,
            threshold,
        }
    }
}

impl<'a> AsBoolField for F64Field<'a> {
    fn get(&self, pos: (isize, isize)) -> bool {
        self.threshold < self.field[pos.0 as usize + pos.1 as usize * self.shape.0 as usize]
    }

    fn shape(&self) -> Shape {
        self.shape
    }
}

pub fn pick_bits<T: AsBoolField>(f: &T, pos: (isize, isize)) -> u8 {
    (f.get_or_false((pos.0, pos.1))) as u8
        | ((f.get_or_false((pos.0 + 1, pos.1)) as u8) << 1)
        | ((f.get_or_false((pos.0 + 1, pos.1 + 1)) as u8) << 2)
        | ((f.get_or_false((pos.0, pos.1 + 1)) as u8) << 3)
}

/// LINE_WIDTH won't work well with cargo fmt
const LW: f32 = 0.4;

/// buffer for vertex shader, use with SliceFlatExt::flat()
pub const CELL_POLYGON_BUFFER: [[f32; 8]; 7] = [
    [1., 1., -1., 1., -1., -1., 1., -1.],
    [1., LW, -1., LW, -1., -LW, 1., -LW],
    [LW, 1., -LW, 1.0, -LW, -1., LW, -1.],
    [-1., -LW, -LW, -1., LW, -1., -1., LW],
    [LW, -1., 1., -LW, 1., LW, -LW, -1.],
    [1., LW, LW, 1., -LW, 1., 1., -LW],
    [-LW, 1., -1., LW, -1., -LW, LW, 1.],
];

/// Index into CELL_POLYGON_BUFFER
pub fn cell_polygon_index(bits: u8) -> i32 {
    match bits {
        1 | 14 => 12,
        2 | 13 => 16,
        4 | 11 => 20,
        8 | 7 => 24,
        3 | 12 => 4,
        9 | 6 => 8,
        _ => 0,
    }
}

/// Whether the pixel is a border
pub(crate) fn _border_pixel(idx: u8) -> bool {
    match idx {
        0 => false,
        1..=14 => true,
        15 => false,
        _ => panic!("index must be in 0-15!"),
    }
}

pub fn cell_lines(bits: u8) -> Vec<[[f32; 2]; 2]> {
    match bits {
        1 | 14 => vec![[[0.5, 0.], [0., 0.5]]],
        2 | 13 => vec![[[0.5, 0.], [1., 0.5]]],
        4 | 11 => vec![[[1., 0.5], [0.5, 1.]]],
        8 | 7 => vec![[[0.5, 1.], [0., 0.5]]],
        3 | 12 => vec![[[1., 0.5], [0., 0.5]]],
        9 | 6 => vec![[[0.5, 1.], [0.5, 0.]]],
        5 | 10 => vec![[[0.5, 0.], [0., 0.5]], [[1., 0.5], [0.5, 1.]]],
        _ => vec![],
    }
}

pub(crate) fn trace_lines(f: &impl AsBoolField) -> Vec<Vec<[usize; 2]>> {
    let shape = f.shape();

    let mut ret: Vec<Vec<[usize; 2]>> = vec![];

    let mut visited = HashMap::new();

    for y in 0..shape.1 {
        for x in 0..shape.0 {
            let pos = (x as isize, y as isize);
            let bits = pick_bits(f, pos);

            if bits == 0 || bits == 15 || visited.get(&[x as usize, y as usize]).is_some() {
                continue;
            }
            let line = trace_line_single(f, [x as usize, y as usize]);
            if let Some(line) = line {
                // println!(
                //     "Got line {:?}, total trace_lines: {}, visited: {}",
                //     line.len(),
                //     ret.len(),
                //     visited.len()
                // );
                let mut duplicate = None;
                for point in &line {
                    if let Some(existing_point) = visited.get(point) {
                        let existing_line: &Vec<_> = &ret[*existing_point];
                        if existing_line.len() < line.len() {
                            duplicate = Some(*existing_point);
                        }
                    }
                }
                if let Some(duplicate) = duplicate {
                    // println!("Duplicate line at {}", duplicate);
                    for point in &line {
                        visited.insert(*point, duplicate);
                    }
                    ret[duplicate] = line;
                } else {
                    // println!("New line at {}", ret.len());
                    for point in &line {
                        visited.insert(*point, ret.len());
                    }
                    ret.push(line);
                }
            }
        }
    }
    ret
}

pub(crate) fn trace_line_single(
    f: &impl AsBoolField,
    mut pos: [usize; 2],
) -> Option<Vec<[usize; 2]>> {
    let mut ret = vec![];

    let mut visited = HashSet::new();
    let mut last_pos: Option<[usize; 2]> = None;

    let mut move_to = |pos: [usize; 2], last_pos: Option<[usize; 2]>, dx: isize, dy: isize| {
        if visited.contains(&(pos, last_pos)) {
            return None;
        }
        ret.push(pos);
        visited.insert((pos, last_pos));
        // println!("find pixel: {:?} len: {}", pos, ret.len());
        let x = pos[0] as isize + dx;
        let y = pos[1] as isize + dy;
        if x < 0 || y < 0 {
            None
        } else {
            Some([x as usize, y as usize])
        }
    };

    loop {
        let bits = pick_bits(f, (pos[0] as isize, pos[1] as isize));
        match bits {
            0 | 15 => return if ret.is_empty() { None } else { Some(ret) },
            1..=14 => {
                let next_pos = match bits {
                    1 => move_to(pos, last_pos, 0, -1),
                    2 => move_to(pos, last_pos, 1, 0),
                    3 => move_to(pos, last_pos, 1, 0),
                    4 => move_to(pos, last_pos, 0, 1),
                    5 => {
                        if let Some(last_pos_val) = last_pos {
                            if last_pos_val[0] < pos[0] {
                                move_to(pos, last_pos, 0, 1)
                            } else {
                                move_to(pos, last_pos, 0, -1)
                            }
                        } else {
                            return None;
                        }
                    }
                    6 => move_to(pos, last_pos, 0, 1),
                    7 => move_to(pos, last_pos, 0, 1),
                    8 => move_to(pos, last_pos, -1, 0),
                    9 => move_to(pos, last_pos, 0, -1),
                    10 => {
                        if let Some(last_pos_val) = last_pos {
                            if last_pos_val[1] < pos[1] {
                                move_to(pos, last_pos, -1, 0)
                            } else {
                                move_to(pos, last_pos, 1, 0)
                            }
                        } else {
                            return None;
                        }
                    }
                    11 => move_to(pos, last_pos, 1, 0),
                    12 => move_to(pos, last_pos, -1, 0),
                    13 => move_to(pos, last_pos, 0, -1),
                    14 => move_to(pos, last_pos, -1, 0),
                    _ => None,
                };
                if let Some(next_pos) = next_pos {
                    last_pos = Some(pos);
                    pos = next_pos;
                } else {
                    return Some(ret);
                }
            }
            _ => return if ret.is_empty() { None } else { Some(ret) },
        }
    }
}

#[test]
fn test_bits() {
    assert_eq!(
        pick_bits(&F64Field::new(&[0., 0., 0., 0.], (2, 2), 0.5), (0, 0)),
        0
    );
    assert_eq!(
        pick_bits(&F64Field::new(&[1., 0., 0., 0.], (2, 2), 0.5), (0, 0)),
        1
    );
    assert_eq!(
        pick_bits(&F64Field::new(&[0., 1., 0., 0.], (2, 2), 0.5), (0, 0)),
        2
    );
    assert_eq!(
        pick_bits(&F64Field::new(&[0., 0., 1., 0.], (2, 2), 0.5), (0, 0)),
        8
    );
    assert_eq!(
        pick_bits(&F64Field::new(&[0., 0., 0., 1.], (2, 2), 0.5), (0, 0)),
        4
    );
}
