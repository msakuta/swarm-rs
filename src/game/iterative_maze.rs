use crate::{
    mesh::{create_mesh, MeshResult},
    perlin_noise::Xor128,
};

use super::{BoardParams, Game};

const FILL_PROB: f64 = 0.4;
const ITERATIONS: usize = 7;
const NARROW_CELL: usize = 2;
const NARROW_THRESHOLD: usize = 12;
const WIDE_CELL: usize = 4;
const WIDE_THRESHOLD: usize = 8;

impl Game {
    pub(super) fn create_iterative_maze_board(params: &BoardParams) -> MeshResult {
        let shape = params.shape;
        let mut xor128 = Xor128::new(params.seed);

        if shape.0 < 2 * WIDE_CELL || shape.1 < 2 * WIDE_CELL {
            return Self::create_perlin_board(params);
        }

        let mut count_wall = 0;
        let mut count_floor = 0;
        let mut board = vec![false; shape.0 * shape.1];
        for y in WIDE_CELL..shape.1 - WIDE_CELL {
            for x in WIDE_CELL..shape.0 - WIDE_CELL {
                let cell = &mut board[x + y * shape.0];
                *cell = !(xor128.next() < FILL_PROB);
                if !*cell {
                    count_wall += 1;
                } else {
                    count_floor += 1;
                }
            }
        }

        let on_board = |x, y| -> bool { x < shape.0 && y < shape.1 };

        let mut temp = board.clone();

        let iterate_map = |board: &[bool], temp: &mut [bool]| {
            let mut count_wall = 0;
            let mut count_floor = 0;
            for y in WIDE_CELL..shape.1 - WIDE_CELL {
                for x in WIDE_CELL..shape.0 - WIDE_CELL {
                    let mut count33 = 0;
                    let mut count55 = 0;

                    for ty in y - NARROW_CELL..=y + NARROW_CELL {
                        for tx in x - NARROW_CELL..=x + NARROW_CELL {
                            if on_board(tx, ty) && !board[ty * shape.0 + tx] {
                                count33 += 1;
                            }
                        }
                    }

                    for ty in y - WIDE_CELL..=y + WIDE_CELL {
                        for tx in x - WIDE_CELL..=x + WIDE_CELL {
                            if on_board(tx, ty) && !board[ty * shape.0 + tx] {
                                count55 += 1;
                            }
                        }
                    }

                    let is_wall = NARROW_THRESHOLD <= count33 || count55 <= WIDE_THRESHOLD;
                    temp[y * shape.0 + x] = !is_wall;
                    if is_wall {
                        count_wall += 1;
                    } else {
                        count_floor += 1;
                    }
                }
            }
        };

        for _i in 0..ITERATIONS {
            iterate_map(&board, &mut temp);
            std::mem::swap(&mut board, &mut temp);
        }

        create_mesh(shape, params.simplify, |xi, yi| {
            *board.get(xi + yi * shape.0).unwrap_or(&false)
        })
    }
}
