use crate::{
    mesh::{create_mesh, MeshResult},
    perlin_noise::{gen_terms, perlin_noise_pixel, Xor128},
};

use super::{BoardParams, Game};

impl Game {
    pub fn create_rooms_board(params: &BoardParams) -> MeshResult {
        let shape = params.shape;

        let mut xor128 = Xor128::new(params.seed);

        let room_rows = shape.0.min(shape.1) * 4 / 128;
        let room_size = shape.0.min(shape.1) / 2 / room_rows;
        let room_sizei = room_size as isize;
        let room_margin = room_size * 2;
        println!("room_rows: {room_rows}, size: {room_size}, margin: {room_margin}");

        let mut board = vec![0f64; shape.0 * shape.1];

        let mut rooms: Vec<(usize, usize)> = vec![];
        for iy in 1..room_rows {
            let yc = (iy * shape.1 / room_rows) as isize;
            for ix in 1..room_rows {
                let xc = (ix * shape.0 / room_rows) as isize;
                for _ in 0..100 {
                    let x = (xor128.nexti() as usize % (room_size * 2)) as isize - room_sizei + xc;
                    let y = (xor128.nexti() as usize % (room_size * 2)) as isize - room_sizei + yc;
                    if !rooms.iter().any(|room| {
                        let dx = room.0 as isize - x;
                        let dy = room.1 as isize - y;
                        let d = dx * dx + dy * dy;
                        d < (room_margin * room_margin) as isize
                    }) {
                        rooms.push((x as usize, y as usize));
                        break;
                    }
                }
            }
        }

        let mut connections: Vec<(usize, usize)> = vec![];
        for (i, room_i) in rooms.iter().enumerate() {
            let mut closest_rooms: Vec<_> = rooms
                .iter()
                .enumerate()
                .filter(|&(j, _)| i != j && connections.iter().all(|&(i2, j2)| i2 != i || j2 != j))
                .map(|(j, room_j)| {
                    let dx = room_i.0 as isize - room_j.0 as isize;
                    let dy = room_i.1 as isize - room_j.1 as isize;
                    let d = dx * dx + dy * dy;
                    (j, room_j, d as usize)
                })
                .collect();
            closest_rooms.sort_by_key(|(_, _, d)| *d);
            let existing_connections = connections.iter().filter(|&&conn| conn.1 == i).count();
            println!("existing_connections[{i}] ({:?}): {existing_connections}, {:?}", room_i, closest_rooms);
            for (closest_j, _closest_room, _) in closest_rooms
                .iter()
                .take(2usize.saturating_sub(existing_connections))
            {
                connections.push((i, *closest_j));
            }
        }

        for &(x, y) in &rooms {
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

        let connection_size = room_size / 3;

        for conn in &connections {
            lerp(rooms[conn.0], rooms[conn.1], |(x, y)| {
                for ix in x.saturating_sub(connection_size)..(x + connection_size).min(shape.0) {
                    for iy in y.saturating_sub(connection_size)..(y + connection_size).min(shape.1)
                    {
                        let dx = ix as isize - x as isize;
                        let dy = iy as isize - y as isize;
                        let d = dx * dx + dy * dy;
                        if d < (connection_size * connection_size) as isize {
                            let val =
                                2. * (1. - d as f64 / (connection_size * connection_size) as f64);
                            board[ix + iy * shape.0] = board[ix + iy * shape.0].max(val);
                        }
                    }
                }
            });
        }

        let min_octave = 2;
        let max_octave = 6;
        let terms = gen_terms(&mut xor128, max_octave);

        create_mesh(shape, params.simplify, |xi, yi| {
            // if board[xi + yi * shape.0] {
            //     return true;
            // }
            // false
            let dx = (xi as isize - shape.0 as isize / 2) as f64;
            let dy = (yi as isize - shape.1 as isize / 2) as f64;
            let noise_val =
                perlin_noise_pixel(xi as f64, yi as f64, min_octave, max_octave, &terms, 0.5);
            let dropoff = (dx * dx + dy * dy).sqrt() / shape.0 as f64;
            board[xi + yi * shape.0] + 3. * noise_val - 0.15 * dropoff > 0.5
        })
    }
}
