use std::collections::{HashMap, HashSet};

use crate::{
    mesh::{create_mesh, MeshResult},
    perlin_noise::{gen_terms, perlin_noise_pixel, Xor128},
};

use super::{BoardParams, Game};

type RoomCoord = (usize, usize);

const ROOM_MODULATION: f64 = 1.;
const NOISE_FACTOR: f64 = 3.;
const DROPOFF_FACTOR: f64 = 0.15;

impl Game {
    pub fn create_rooms_board(params: &BoardParams) -> MeshResult {
        let shape = params.shape;

        let mut xor128 = Xor128::new(params.seed);

        let room_rows = shape.0.min(shape.1) * 4 / 128;
        if room_rows <= 1 {
            return Self::create_perlin_board(params);
        }
        let room_size = shape.0.min(shape.1) / 2 / room_rows;
        let room_margin = room_size * 2;
        println!("room_rows: {room_rows}, size: {room_size}, margin: {room_margin}");

        let mut board = vec![0f64; shape.0 * shape.1];

        let mut rooms = HashMap::<RoomCoord, RoomCoord>::new();
        for iy in 1..room_rows {
            let yc = (iy * shape.1 / room_rows) as i32;
            for ix in 1..room_rows {
                let xc = (ix * shape.0 / room_rows) as i32;
                for _ in 0..100 {
                    let x = (xor128.next() - 0.5) * ROOM_MODULATION * room_size as f64 + xc as f64;
                    let y = (xor128.next() - 0.5) * ROOM_MODULATION * room_size as f64 + yc as f64;
                    if !rooms.values().any(|room| {
                        let dx = room.0 as f64 - x;
                        let dy = room.1 as f64 - y;
                        let d = dx * dx + dy * dy;
                        d < (room_margin * room_margin) as f64
                    }) {
                        rooms.insert((ix, iy), (x.max(0.) as usize, y.max(0.) as usize));
                        break;
                    }
                }
            }
        }

        let mut connections = HashSet::<(RoomCoord, RoomCoord)>::new();
        let mut open_ends = vec![(room_rows / 2, room_rows / 2)];
        loop {
            if open_ends.is_empty() {
                break;
            }
            let Some(&room_i) = open_ends.iter().nth(xor128.nexti() as usize % open_ends.len()) else {
                break
            };
            let xi = room_i.0 as i32;
            let yi = room_i.1 as i32;
            const CONNECTION_DIRECTIONS: [[i32; 2]; 4] = [[-1, 0], [0, -1], [1, 0], [0, 1]];
            let mut available_connection_bits: u32 = 0xf;
            for &(j1, j2) in &connections {
                let other = if room_i == j1 { j2 } else { j1 };
                let xj = other.0 as i32;
                let yj = other.1 as i32;
                for (bit, dir) in CONNECTION_DIRECTIONS.iter().enumerate() {
                    if xi + dir[0] == xj && yi + dir[1] == yj {
                        available_connection_bits &= !(1 << bit);
                    }
                }
            }
            for (bit, dir) in CONNECTION_DIRECTIONS.iter().enumerate() {
                let xj = xi + dir[0];
                let yj = yi + dir[1];
                if xj <= 0 || room_rows as i32 <= xj || yj <= 0 || room_rows as i32 <= yj {
                    available_connection_bits &= !(1 << bit);
                }
            }
            if available_connection_bits == 0 {
                if let Some((idx, _)) = open_ends.iter().enumerate().find(|(_, &idx)| idx == room_i)
                {
                    open_ends.swap_remove(idx);
                    continue;
                } else {
                    break;
                }
            }
            let available_connections: Vec<_> = (0..4)
                .filter(|i| available_connection_bits & (1 << i) != 0)
                .map(|i| CONNECTION_DIRECTIONS[i])
                .collect();
            let conn_i = xor128.nexti() as usize % available_connections.len();
            let conn = available_connections[conn_i];
            let xj = xi + conn[0];
            let yj = yi + conn[1];
            let room_j = (xj as usize, yj as usize);
            if room_i < room_j {
                connections.insert((room_i, room_j));
            } else {
                connections.insert((room_j, room_i));
            }
            if !open_ends.iter().any(|&room_k| room_k == room_j) {
                open_ends.push(room_j);
            }
        }

        let connection_size = room_size / 3;

        // We need to sort by room key first to get reproduceable map from the seed,
        // because HashMap has internal randomness that affects iteration order.
        let mut room_keys = rooms.keys().collect::<Vec<_>>();
        room_keys.sort();

        for key in room_keys {
            let room_size = if xor128.next() < 0.5 {
                connection_size
            } else {
                room_size
            };
            let &(x, y) = rooms.get(key).unwrap();
            for ix in x.saturating_sub(room_size)..(x + room_size).min(shape.0) {
                for iy in y.saturating_sub(room_size)..(y + room_size).min(shape.1) {
                    let dx = ix as isize - x as isize;
                    let dy = iy as isize - y as isize;
                    let d = dx * dx + dy * dy;
                    if d < (room_size * room_size) as isize {
                        let val = 2. * (1. - d as f64 / (room_size * room_size) as f64);
                        board[ix + iy * shape.0] = board[ix + iy * shape.0].max(val);
                    }
                }
            }
        }

        fn lerp(a: (usize, usize), b: (usize, usize), mut fun: impl FnMut((usize, usize))) {
            let dx = a.0 as isize - b.0 as isize;
            let dy = a.1 as isize - b.1 as isize;
            let interpolates = dx.abs().max(dy.abs());
            for i in 0..=interpolates {
                let f = i as f64 / interpolates as f64;
                fun((
                    (a.0 as f64 * (1. - f) + b.0 as f64 * f) as usize,
                    (a.1 as f64 * (1. - f) + b.1 as f64 * f) as usize,
                ));
            }
        }

        for conn in &connections {
            if let Some((room_i, room_j)) = rooms.get(&conn.0).zip(rooms.get(&conn.1)) {
                lerp(*room_i, *room_j, |(x, y)| {
                    for ix in x.saturating_sub(connection_size)..(x + connection_size).min(shape.0)
                    {
                        for iy in
                            y.saturating_sub(connection_size)..(y + connection_size).min(shape.1)
                        {
                            let dx = ix as isize - x as isize;
                            let dy = iy as isize - y as isize;
                            let d = dx * dx + dy * dy;
                            if d < (connection_size * connection_size) as isize {
                                let val = 2.
                                    * (1. - d as f64 / (connection_size * connection_size) as f64);
                                board[ix + iy * shape.0] = board[ix + iy * shape.0].max(val);
                            }
                        }
                    }
                });
            }
        }

        let min_octave = 2;
        let max_octave = 6;
        let terms = gen_terms(&mut xor128, max_octave);

        create_mesh(shape, params.simplify, |xi, yi| {
            let dx = (xi as isize - shape.0 as isize / 2) as f64;
            let dy = (yi as isize - shape.1 as isize / 2) as f64;
            let noise_val =
                perlin_noise_pixel(xi as f64, yi as f64, min_octave, max_octave, &terms, 0.5);
            let dropoff = (dx * dx + dy * dy).sqrt() / shape.0 as f64;
            board[xi + yi * shape.0] + NOISE_FACTOR * noise_val - DROPOFF_FACTOR * dropoff > 0.5
        })
    }
}
