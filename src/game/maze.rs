use super::{BoardParams, Game};
use crate::{
    dijkstra,
    mesh::{create_mesh, MeshResult},
    perlin_noise::Xor128,
};

impl Game {
    /// An algorithm to generate a maze-like map with some amount of random noise.
    ///
    /// It ensures the free map as one connected region, unlike the one with Perlin noise.
    /// It uses multiple resolution and distance map to create a maze-like structure and randomness on the border.
    /// The resolutions are power of 2 to make it better fit to QTree.
    pub fn create_maze_board(params: &BoardParams) -> MeshResult {
        let shape = params.shape;
        let mut board = vec![false; shape.0 * shape.1];
        for resolution in [8, 4, 2, 1] {
            let maze_shape = (shape.0 / resolution, shape.1 / resolution);
            let mut maze_board = vec![false; maze_shape.0 * maze_shape.1];

            // Downsample
            for ix in 0..maze_shape.0 {
                for iy in 0..maze_shape.1 {
                    maze_board[ix + iy * maze_shape.1] =
                        board[ix * resolution + iy * resolution * shape.1];
                }
            }

            maze_board[maze_shape.0 / 2 + (maze_shape.1 / 2) * maze_shape.0] = true;

            let mut rand = Xor128::new(params.seed);

            let pick_pixel = |board: &[bool], free: bool, rand: &mut Xor128| {
                let filter_cond = |&(i, b): &(usize, &bool)| {
                    let x = i % maze_shape.0;
                    let y = i / maze_shape.0;
                    if resolution == 1 {
                        *b == free
                    } else {
                        x % 2 == 0 && y % 2 == 0 && *b == free
                    }
                };
                let free_pixels = board.iter().enumerate().filter(filter_cond).count();
                let ret = if free_pixels == 0 {
                    maze_shape.0 / 2 + (maze_shape.1 / 2) * maze_shape.0
                } else {
                    let idx = rand.nexti() % free_pixels as u32;
                    board
                        .iter()
                        .enumerate()
                        .filter(filter_cond)
                        .enumerate()
                        .nth(idx as usize)
                        .unwrap()
                        .1
                         .0
                };
                [(ret % maze_shape.0) as i32, (ret / maze_shape.0) as i32]
            };

            // The costmap is used to proritize unexplored areas of the map (having larger
            // distance from origin) to make the map more maze-like and "interesting".
            // We use costmap only in larger resolutions, because Dijkstra takes a bit of time
            // and we don't care too much about prioritizing unexplored area in lower resolutions,
            // where the general structure of the map is already built in larger resolutions.
            let mut costmap = if 2 < resolution {
                let mut costmap = vec![std::i32::MAX; maze_board.len()];
                let start = [(maze_shape.0 / 2) as i32, (maze_shape.1 / 2) as i32];
                dijkstra::dijkstra_fill(&maze_board, maze_shape, start, &mut costmap);
                Some(costmap)
            } else {
                None
            };

            let pick_pixel_cost =
                |board: &[bool], costmap: &[i32], free: bool, rand: &mut Xor128| {
                    let filter_cond = |&(i, b): &(usize, &bool)| {
                        let x = i % maze_shape.0;
                        let y = i / maze_shape.0;
                        if resolution == 1 {
                            *b == free
                        } else {
                            x % 2 == 0 && y % 2 == 0 && *b == free
                        }
                    };
                    let cost_func = |(i, _)| {
                        let dx = (i % shape.0) as i32 - (shape.0 / 2) as i32;
                        let dy = (i / shape.0) as i32 - (shape.1 / 2) as i32;
                        let factor =
                            ((shape.0 / 2) as i32 - dx.abs()).min((shape.1 / 2) as i32 - dy.abs());
                        let factor = factor.min(20);
                        let factor = 1;
                        let cost = costmap[i] + 1;
                        (i, (cost * cost * factor) as i32)
                    };
                    let total_weights: i32 = board
                        .iter()
                        .enumerate()
                        .filter(filter_cond)
                        .map(|i| cost_func(i).1)
                        .sum();
                    let ret = if total_weights == 0 {
                        maze_shape.0 / 2 + (maze_shape.1 / 2) * maze_shape.0
                    } else {
                        let idx = rand.nexti() % total_weights as u32;
                        let mut accum = 0;
                        'found: {
                            for (i, cost) in
                                board.iter().enumerate().filter(filter_cond).map(cost_func)
                            {
                                if idx < accum {
                                    break 'found i;
                                }
                                accum += cost as u32;
                            }
                            return [maze_shape.0 as i32 / 2, maze_shape.1 as i32 / 2];
                        }
                    };
                    [(ret % maze_shape.0) as i32, (ret / maze_shape.0) as i32]
                };

            const DIRECTIONS: [[i32; 2]; 4] = [[-1, 0], [0, -1], [1, 0], [0, 1]];

            for _ in 0..params.maze_expansions / resolution / resolution {
                let source = if let Some(costmap) = &costmap {
                    pick_pixel_cost(&maze_board, &costmap, true, &mut rand)
                } else {
                    pick_pixel(&maze_board, true, &mut rand)
                };
                let dir = DIRECTIONS[(rand.nexti() % 4) as usize];
                let mut next = source;
                for _ in 0..2 {
                    let next_next = [next[0] + dir[0], next[1] + dir[1]];
                    if next_next[0] < 0
                        || maze_shape.0 as i32 <= next_next[0]
                        || next_next[1] < 0
                        || maze_shape.1 as i32 <= next_next[1]
                    {
                        continue;
                    }
                    let next_idx = next_next[0] as usize + next_next[1] as usize * maze_shape.0;
                    maze_board[next_idx] = true;
                    if let Some(costmap) = &mut costmap {
                        // A heuristic cost estimation. This is not accurate, since a connection can make a lot of pixels
                        // change cost, but we don't want to repeat Dijkstra every time.
                        costmap[next_idx] =
                            costmap[next[0] as usize + next[1] as usize * maze_shape.0] + 1;
                    }
                    next = next_next;
                }
            }

            // Upsample
            for ix in 0..shape.0 {
                for iy in 0..shape.1 {
                    board[ix + iy * shape.0] =
                        maze_board[ix / resolution + iy / resolution * maze_shape.0];
                }
            }
        }

        create_mesh(shape, params.simplify, |xi, yi| {
            *board.get(xi + yi * shape.0).unwrap_or(&false)
        })
    }
}
