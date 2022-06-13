use crate::shape::Shape;

/// A trait for objects that can behave like a field of booleans.
pub(crate) trait AsBoolField {
    fn get(&self, pos: (isize, isize)) -> bool;
}

/// A concrete type of a boolean field.
#[derive(Clone, Copy)]
pub(crate) struct BoolField<'a> {
    field: &'a [bool],
    shape: Shape,
}

impl<'a> BoolField<'a> {
    pub(crate) fn new(field: &'a [bool], shape: Shape) -> Self {
        Self { field, shape }
    }
}

impl<'a> AsBoolField for BoolField<'a> {
    fn get(&self, pos: (isize, isize)) -> bool {
        self.field[pos.0 as usize + pos.1 as usize * self.shape.0 as usize]
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
}

pub(crate) fn pick_bits<T: AsBoolField>(f: T, pos: (isize, isize)) -> u8 {
    (f.get((pos.0, pos.1))) as u8
        | ((f.get((pos.0 + 1, pos.1)) as u8) << 1)
        | ((f.get((pos.0 + 1, pos.1 + 1)) as u8) << 2)
        | ((f.get((pos.0, pos.1 + 1)) as u8) << 3)
}

/// LINE_WIDTH won't work well with cargo fmt
const LW: f32 = 0.4;

/// buffer for vertex shader, use with SliceFlatExt::flat()
pub(crate) const CELL_POLYGON_BUFFER: [[f32; 8]; 7] = [
    [1., 1., -1., 1., -1., -1., 1., -1.],
    [1., LW, -1., LW, -1., -LW, 1., -LW],
    [LW, 1., -LW, 1.0, -LW, -1., LW, -1.],
    [-1., -LW, -LW, -1., LW, -1., -1., LW],
    [LW, -1., 1., -LW, 1., LW, -LW, -1.],
    [1., LW, LW, 1., -LW, 1., 1., -LW],
    [-LW, 1., -1., LW, -1., -LW, LW, 1.],
];

/// Index into CELL_POLYGON_BUFFER
pub(crate) fn cell_polygon_index(bits: u8) -> i32 {
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

pub(crate) fn cell_lines(bits: u8) -> Vec<[[f32; 2]; 2]> {
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

#[test]
fn test_bits() {
    assert_eq!(
        pick_bits(F64Field::new(&[0., 0., 0., 0.], (2, 2), 0.5), (0, 0)),
        0
    );
    assert_eq!(
        pick_bits(F64Field::new(&[1., 0., 0., 0.], (2, 2), 0.5), (0, 0)),
        1
    );
    assert_eq!(
        pick_bits(F64Field::new(&[0., 1., 0., 0.], (2, 2), 0.5), (0, 0)),
        2
    );
    assert_eq!(
        pick_bits(F64Field::new(&[0., 0., 1., 0.], (2, 2), 0.5), (0, 0)),
        8
    );
    assert_eq!(
        pick_bits(F64Field::new(&[0., 0., 0., 1.], (2, 2), 0.5), (0, 0)),
        4
    );
}
