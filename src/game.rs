use cgmath::{InnerSpace, Vector2};
use delaunator::{triangulate, Triangulation};
use druid::{piet::kurbo::BezPath, Data, Point};
use std::{cell::RefCell, collections::HashMap, rc::Rc};

use crate::{
    agent::{Agent, Bullet, AGENT_HALFLENGTH, AGENT_HALFWIDTH},
    app_data::is_passable_at,
    entity::{Entity, GameEvent},
    marching_squares::{trace_lines, BoolField},
    measure_time,
    perlin_noise::{gen_terms, perlin_noise_pixel, Xor128},
    spawner::Spawner,
    triangle_utils::{center_of_triangle_obj, find_triangle_at, label_triangles},
};

pub(crate) type Board = Vec<bool>;

#[derive(Debug, Clone, Data)]
pub(crate) struct Profiler {
    total: f64,
    count: usize,
}

impl Profiler {
    pub(crate) fn new() -> Self {
        Self {
            total: 0.,
            count: 0,
        }
    }

    pub(crate) fn get_average(&self) -> f64 {
        if self.count == 0 {
            0.
        } else {
            self.total / self.count as f64
        }
    }

    pub(crate) fn get_count(&self) -> usize {
        self.count
    }

    pub(crate) fn add(&mut self, sample: f64) {
        self.total += sample;
        self.count += 1;
    }
}

#[derive(Debug, Clone, Data)]
pub(crate) struct Game {
    pub(crate) xs: usize,
    pub(crate) ys: usize,
    pub(crate) simplify: f64,
    pub(crate) board: Rc<Board>,
    pub(crate) simplified_border: Rc<Vec<BezPath>>,
    pub(crate) triangulation: Rc<Triangulation>,
    pub(crate) triangle_passable: Rc<Vec<bool>>,
    pub(crate) triangle_labels: Rc<Vec<i32>>,
    pub(crate) points: Rc<Vec<delaunator::Point>>,
    pub(crate) largest_label: Option<i32>,
    pub(crate) entities: Rc<RefCell<Vec<RefCell<Entity>>>>,
    pub(crate) bullets: Rc<Vec<Bullet>>,
    pub(crate) paused: bool,
    pub(crate) interval: f64,
    pub(crate) rng: Rc<Xor128>,
    pub(crate) id_gen: usize,
    pub(crate) triangle_profiler: Profiler,
    pub(crate) pixel_profiler: Rc<RefCell<Profiler>>,
    pub(crate) source: Rc<String>,
}

impl Game {
    pub(crate) fn new() -> Self {
        let seed = 123513;
        let simplify = 1.;

        let xs = 128;
        let ys = 128;

        let (board, simplified_border, points) = Self::create_board((xs, ys), seed, simplify);

        let triangulation = triangulate(&points);

        let triangle_passable =
            Self::calc_passable_triangles(&board, (xs, ys), &points, &triangulation);

        let triangle_labels = label_triangles(&triangulation, &triangle_passable);

        let mut label_stats = HashMap::new();
        for label in &triangle_labels {
            if *label != -1 {
                *label_stats.entry(*label).or_insert(0) += 1;
            }
        }
        let largest_label = label_stats
            .iter()
            .max_by_key(|(_, count)| **count)
            .map(|(key, _)| *key);

        let id_gen = 0;

        Self {
            xs,
            ys,
            simplify,
            board: Rc::new(board),
            simplified_border: Rc::new(simplified_border),
            triangulation: Rc::new(triangulation),
            triangle_passable: Rc::new(triangle_passable),
            triangle_labels: Rc::new(triangle_labels),
            largest_label,
            points: Rc::new(points),
            entities: Rc::new(RefCell::new(vec![])),
            bullets: Rc::new(vec![]),
            paused: false,
            interval: 32.,
            rng: Rc::new(Xor128::new(9318245)),
            id_gen,
            triangle_profiler: Profiler::new(),
            pixel_profiler: Rc::new(RefCell::new(Profiler::new())),
            source: Rc::new(String::new()),
        }
    }

    pub fn create_board(
        (xs, ys): (usize, usize),
        seed: u32,
        simplify_epsilon: f64,
    ) -> (Vec<bool>, Vec<BezPath>, Vec<delaunator::Point>) {
        let bits = 6;
        let mut xor128 = Xor128::new(seed);
        let terms = gen_terms(&mut xor128, bits);

        let mut board = vec![false; xs * ys];
        for (i, cell) in board.iter_mut().enumerate() {
            let xi = i % xs;
            let yi = i / ys;
            let dx = (xi as isize - xs as isize / 2) as f64;
            let dy = (yi as isize - ys as isize / 2) as f64;
            let noise_val = perlin_noise_pixel(xi as f64, yi as f64, bits, &terms, 0.5);
            *cell = 0. + (noise_val - 0.5 * (dx * dx + dy * dy).sqrt() / xs as f64) > -0.125;
        }

        println!(
            "true: {}, false: {}",
            board.iter().filter(|c| **c).count(),
            board.iter().filter(|c| !**c).count()
        );

        let shape = (xs as isize, ys as isize);

        let field = BoolField::new(&board, shape);

        let mut simplified_border = vec![];
        let mut points = vec![];

        let to_point = |p: [f64; 2]| Point::new(p[0] as f64, p[1] as f64);

        let lines = trace_lines(&field);
        let mut simplified_vertices = 0;
        for line in &lines {
            let simplified = if simplify_epsilon == 0. {
                line.iter().map(|p| [p[0] as f64, p[1] as f64]).collect()
            } else {
                // println!("rdp closed: {} start/end: {:?}/{:?}", line.first() == line.last(), line.first(), line.last());

                // if the ring is closed, remove the last element to open it, because rdp needs different start and end points
                let mut slice = &line[..];
                while 1 < slice.len() && slice.first() == slice.last() {
                    slice = &slice[..slice.len() - 1];
                }

                crate::rdp::rdp(
                    &slice
                        .iter()
                        .map(|p| [p[0] as f64, p[1] as f64])
                        .collect::<Vec<_>>(),
                    simplify_epsilon,
                )
            };

            // If the polygon does not make up a triangle, skip it
            if simplified.len() <= 2 {
                continue;
            }

            if let Some((first, rest)) = simplified.split_first() {
                let mut bez_path = BezPath::new();
                bez_path.move_to(to_point(*first));
                for point in rest {
                    bez_path.line_to(to_point(*point));
                    points.push(delaunator::Point {
                        x: point[0],
                        y: point[1],
                    });
                }
                bez_path.close_path();
                simplified_border.push(bez_path);
                simplified_vertices += simplified.len();
            }
        }
        println!(
            "trace_lines: {}, vertices: {}, simplified_border: {} vertices: {}",
            lines.len(),
            lines.iter().map(|line| line.len()).sum::<usize>(),
            simplified_border.len(),
            simplified_vertices
        );

        (board, simplified_border, points)
    }

    pub(crate) fn calc_passable_triangles(
        board: &[bool],
        shape: (usize, usize),
        points: &[delaunator::Point],
        triangulation: &Triangulation,
    ) -> Vec<bool> {
        triangulation
            .triangles
            .chunks(3)
            .enumerate()
            .map(|(t, _)| {
                let pos = center_of_triangle_obj(&triangulation, points, t);
                is_passable_at(&board, shape, [pos.x, pos.y])
            })
            .collect()
    }

    pub(crate) fn new_board(&mut self, shape: (usize, usize), seed: u32, simplify: f64) {
        self.xs = shape.0;
        self.ys = shape.0;
        let (board, simplified_border, points) = Self::create_board(shape, seed, simplify);

        let triangulation = triangulate(&points);
        let triangle_passable =
            Self::calc_passable_triangles(&board, shape, &points, &triangulation);

        let triangle_labels = label_triangles(&triangulation, &triangle_passable);

        let mut label_stats = HashMap::new();
        for label in &triangle_labels {
            if *label != -1 {
                *label_stats.entry(*label).or_insert(0) += 1;
            }
        }
        self.largest_label = label_stats
            .iter()
            .max_by_key(|(_, count)| **count)
            .map(|(key, _)| *key);

        self.board = Rc::new(board);
        self.simplified_border = Rc::new(simplified_border);
        self.triangulation = Rc::new(triangulation);
        self.points = Rc::new(points);
        self.triangle_passable = Rc::new(triangle_passable);
        self.triangle_labels = Rc::new(triangle_labels);
        self.entities = Rc::new(RefCell::new(vec![]));
        self.bullets = Rc::new(vec![]);
    }

    pub(crate) fn try_new_agent(
        &mut self,
        pos: [f64; 2],
        team: usize,
        entities: &[RefCell<Entity>],
    ) -> Option<Entity> {
        let rng = Rc::make_mut(&mut self.rng);
        let id_gen = &mut self.id_gen;
        let triangulation = &self.triangulation;
        let points = &self.points;
        let triangle_labels = &self.triangle_labels;
        let largest_label = self.largest_label;
        for _ in 0..10 {
            let pos_candidate = [
                pos[0] + rng.next() * 10. - 5.,
                pos[1] + rng.next() * 10. - 5.,
            ];

            if Agent::collision_check(None, pos_candidate, entities) {
                continue;
            }

            let orient_candidate = rng.next() * std::f64::consts::PI * 2.;
            if let Some(tri) = find_triangle_at(
                &triangulation,
                &points,
                pos_candidate,
                &mut self.triangle_profiler,
            ) {
                if Some(triangle_labels[tri]) == largest_label {
                    return Some(Entity::Agent(Agent::new(
                        id_gen,
                        pos_candidate,
                        orient_candidate,
                        team,
                        &self.source,
                    )));
                }
            }
        }
        None
    }

    fn try_new_spawner(&mut self, team: usize) -> Option<Entity> {
        for _ in 0..10 {
            let rng = Rc::make_mut(&mut self.rng);
            let pos_candidate = [rng.next() * self.xs as f64, rng.next() * self.ys as f64];
            if let Some(tri) = find_triangle_at(
                &self.triangulation,
                &self.points,
                pos_candidate,
                &mut self.triangle_profiler,
            ) {
                if Some(self.triangle_labels[tri]) == self.largest_label {
                    if self.board[pos_candidate[0] as usize + self.xs * pos_candidate[1] as usize] {
                        return Some(Entity::Spawner(Spawner::new(
                            &mut self.id_gen,
                            pos_candidate,
                            team,
                        )));
                    }
                }
            }
        }
        None
    }

    pub(crate) fn update(&mut self) {
        let mut entities = std::mem::take(&mut *self.entities.borrow_mut());
        let mut bullets = std::mem::take(Rc::make_mut(&mut self.bullets));
        let mut events = vec![];
        for entity in entities.iter() {
            let mut entity = entity.borrow_mut();
            events.extend(entity.update(self, &entities, &mut bullets));
        }

        for event in events {
            match event {
                GameEvent::SpawnAgent { pos, team } => {
                    if let Some(agent) = self.try_new_agent(pos, team, &entities) {
                        entities.push(RefCell::new(agent));
                    }
                }
            }
        }

        *self.entities.borrow_mut() = entities;
        self.bullets = Rc::new(bullets);

        {
            let agents = self.entities.as_ref().borrow();
            self.bullets = Rc::new(
                self.bullets
                    .iter()
                    .filter_map(|bullet| {
                        if !self.is_passable_at(bullet.pos) {
                            return None;
                        }
                        let newpos =
                            (Vector2::from(bullet.pos) + Vector2::from(bullet.velo)).into();
                        for agent in agents.iter() {
                            let mut agent = agent.borrow_mut();
                            if agent.get_team() == bullet.team {
                                continue;
                            }
                            let agent_pos = Vector2::from(agent.get_pos());
                            let agent_vertices = [
                                [
                                    agent_pos.x - AGENT_HALFLENGTH,
                                    agent_pos.y - AGENT_HALFWIDTH,
                                ],
                                [
                                    agent_pos.x - AGENT_HALFLENGTH,
                                    agent_pos.y + AGENT_HALFWIDTH,
                                ],
                                [
                                    agent_pos.x + AGENT_HALFLENGTH,
                                    agent_pos.y + AGENT_HALFWIDTH,
                                ],
                                [
                                    agent_pos.x + AGENT_HALFLENGTH,
                                    agent_pos.y - AGENT_HALFWIDTH,
                                ],
                            ];
                            if separating_axis(
                                &Vector2::from(bullet.pos),
                                &Vector2::from(bullet.velo),
                                agent_vertices.into_iter().map(Vector2::from),
                            ) {
                                if !agent.damage() {
                                    agent.set_active(false);
                                }
                                println!("Agent {} is being killed", agent.get_id());
                                return None;
                            }
                        }
                        let mut ret = bullet.clone();
                        ret.pos = newpos;
                        ret.traveled += Vector2::from(bullet.velo).magnitude();
                        Some(ret)
                    })
                    .collect(),
            );
        }

        let mut entities: Vec<_> = std::mem::take(&mut *self.entities.borrow_mut())
            .into_iter()
            .filter(|agent| agent.borrow().get_active())
            .collect();

        for team in 0..2 {
            let rng = Rc::make_mut(&mut self.rng);
            if entities
                .iter()
                .filter(|agent| !agent.borrow().is_agent() && agent.borrow().get_team() == team)
                .count()
                < 1
                && rng.next() < 0.1
            {
                if let Some(spawner) = self.try_new_spawner(team) {
                    entities.push(RefCell::new(spawner));
                }
            }
        }
        *self.entities.borrow_mut() = entities;
    }

    pub(crate) fn is_passable_at(&self, pos: [f64; 2]) -> bool {
        if pos[0] < 0. || self.xs <= pos[0] as usize || pos[1] < 0. || self.ys <= pos[1] as usize {
            false
        } else {
            let (ret, time) =
                measure_time(|| self.board[pos[0] as usize + pos[1] as usize * self.xs]);
            self.pixel_profiler.borrow_mut().add(time);
            ret
        }
    }

    // pub(crate) fn is_passable_at(board: &[bool], shape: (usize, usize), pos: [f64; 2]) -> bool {
    //     if pos[0] < 0. || shape.0 <= pos[0] as usize || pos[1] < 0. || shape.1 <= pos[1] as usize {
    //         false
    //     } else {
    //         board[pos[0] as usize + pos[1] as usize * shape.0]
    //     }
    // }
}

fn separating_axis(
    org: &Vector2<f64>,
    dir: &Vector2<f64>,
    polygon: impl Iterator<Item = Vector2<f64>>,
) -> bool {
    let xhat = dir.normalize();
    let yhat = Vector2::new(xhat.y, -xhat.x);

    if let Some(bbox) = polygon.fold(None, |acc: Option<[f64; 4]>, vertex| {
        let x = xhat.dot(vertex - org);
        let y = yhat.dot(vertex - org);
        if let Some(acc) = acc {
            Some([acc[0].min(x), acc[1].min(y), acc[2].max(x), acc[3].max(y)])
        } else {
            Some([x, y, x, y])
        }
    }) {
        0. < bbox[2] && bbox[0] < dir.magnitude() && 0. < bbox[3] && bbox[1] < 0.
    } else {
        false
    }
}
