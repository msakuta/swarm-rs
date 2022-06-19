use crate::{
    agent::{Agent, Bullet},
    entity::{Entity, GameEvent},
    marching_squares::{trace_lines, BoolField},
    perlin_noise::{gen_terms, perlin_noise_pixel, Xor128},
    spawner::Spawner,
    triangle_utils::{center_of_triangle_obj, find_triangle_at, label_triangles},
    WINDOW_HEIGHT,
};
use ::cgmath::{MetricSpace, Vector2};
use ::delaunator::{triangulate, Triangulation};
use druid::{piet::kurbo::BezPath, Data, Lens, Point, Vec2};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
};

#[derive(Clone, PartialEq, Eq, Data)]
pub(crate) enum LineMode {
    None,
    Line,
    Polygon,
}

type Board = Vec<bool>;

#[derive(Clone, Lens, Data)]
pub(crate) struct AppData {
    pub(crate) rows_text: String,
    pub(crate) columns_text: String,
    pub(crate) seed_text: String,
    pub(crate) simplify_text: String,
    pub(crate) xs: usize,
    pub(crate) ys: usize,
    pub(crate) board: Rc<Board>,
    pub(crate) line_mode: LineMode,
    pub(crate) simplified_border: Rc<Vec<BezPath>>,
    pub(crate) simplified_visible: bool,
    pub(crate) triangulation: Rc<Triangulation>,
    pub(crate) points: Rc<Vec<delaunator::Point>>,
    pub(crate) largest_label: Option<i32>,
    pub(crate) triangle_passable: Rc<Vec<bool>>,
    pub(crate) triangle_labels: Rc<Vec<i32>>,
    pub(crate) triangulation_visible: bool,
    pub(crate) unpassable_visible: bool,
    pub(crate) triangle_label_visible: bool,
    pub(crate) origin: Vec2,
    pub(crate) scale: f64,
    pub(crate) message: String,
    pub(crate) get_board_time: f64,
    #[data(ignore)]
    pub(crate) render_board_time: Cell<f64>,
    pub(crate) render_stats: Rc<RefCell<String>>,
    pub(crate) entities: Rc<Vec<RefCell<Entity>>>,
    pub(crate) bullets: Rc<Vec<Bullet>>,
    pub(crate) paused: bool,
    pub(crate) interval: f64,
    pub(crate) rng: Rc<Xor128>,
    pub(crate) id_gen: usize,
    pub(crate) path_visible: bool,
    pub(crate) target_visible: bool,
}

impl AppData {
    pub(crate) fn new() -> Self {
        let seed = 123513;
        let simplify = 1.;

        let xs = 128;
        let ys = 128;

        let w = 32.;

        let (board, simplified_border, points) = AppData::create_board((xs, ys), seed, simplify);

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
            rows_text: xs.to_string(),
            columns_text: ys.to_string(),
            seed_text: seed.to_string(),
            simplify_text: simplify.to_string(),
            xs,
            ys,
            board: Rc::new(board),
            line_mode: LineMode::None,
            simplified_border: Rc::new(simplified_border),
            simplified_visible: false,
            triangulation: Rc::new(triangulation),
            triangle_passable: Rc::new(triangle_passable),
            triangle_labels: Rc::new(triangle_labels),
            largest_label,
            points: Rc::new(points),
            triangulation_visible: false,
            unpassable_visible: false,
            triangle_label_visible: false,
            origin: Vec2::new(0., 0.),
            scale: WINDOW_HEIGHT / w / ys as f64,
            message: "".to_string(),
            render_board_time: Cell::new(0.),
            get_board_time: 0.,
            render_stats: Rc::new(RefCell::new("".to_string())),
            entities: Rc::new(vec![]),
            bullets: Rc::new(vec![]),
            paused: false,
            interval: 100.,
            rng: Rc::new(Xor128::new(9318245)),
            id_gen,
            path_visible: true,
            target_visible: true,
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
        let (board, simplified_border, points) = AppData::create_board(shape, seed, simplify);

        let triangulation = triangulate(&points);
        let triangle_passable =
            AppData::calc_passable_triangles(&board, shape, &points, &triangulation);

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
        self.entities = Rc::new(vec![]);
        self.bullets = Rc::new(vec![]);
    }

    pub(crate) fn try_new_agent(&mut self, pos: [f64; 2], team: usize) -> Option<Entity> {
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
            if let Some(tri) = find_triangle_at(&triangulation, &points, pos_candidate) {
                if Some(triangle_labels[tri]) == largest_label {
                    return Some(Entity::Agent(Agent::new(id_gen, pos_candidate, team)));
                }
            }
        }
        None
    }

    fn try_new_spawner(&mut self, team: usize) -> Option<Entity> {
        for _ in 0..10 {
            let rng = Rc::make_mut(&mut self.rng);
            let pos_candidate = [rng.next() * self.xs as f64, rng.next() * self.ys as f64];
            if let Some(tri) = find_triangle_at(&self.triangulation, &self.points, pos_candidate) {
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
        let mut entities = std::mem::take(Rc::make_mut(&mut self.entities));
        let mut bullets = std::mem::take(Rc::make_mut(&mut self.bullets));
        let mut events = vec![];
        for entity in entities.iter() {
            let mut entity = entity.borrow_mut();
            events.extend(entity.update(self, &entities, &mut bullets));
        }

        for event in events {
            match event {
                GameEvent::SpawnAgent { pos, team } => {
                    if let Some(agent) = self.try_new_agent(pos, team) {
                        entities.push(RefCell::new(agent));
                    }
                }
            }
        }

        self.entities = Rc::new(entities);
        self.bullets = Rc::new(bullets);

        let agents = &self.entities;
        self.bullets = Rc::new(
            self.bullets
                .iter()
                .filter_map(|bullet| {
                    if !self.is_passable_at(bullet.pos) {
                        return None;
                    }
                    let newpos = (Vector2::from(bullet.pos) + Vector2::from(bullet.velo)).into();
                    for agent in agents.iter() {
                        let mut agent = agent.borrow_mut();
                        if agent.get_team() == bullet.team {
                            continue;
                        }
                        let dist2 = Vector2::from(agent.get_pos()).distance2(Vector2::from(newpos));
                        if dist2 < 3. * 3. {
                            if !agent.damage() {
                                agent.set_active(false);
                            }
                            println!("Agent {} is being killed", agent.get_id());
                            return None;
                        }
                    }
                    let mut ret = bullet.clone();
                    ret.pos = newpos;
                    Some(ret)
                })
                .collect(),
        );

        let mut entities: Vec<_> = std::mem::take(Rc::make_mut(&mut self.entities))
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
        self.entities = Rc::new(entities);
    }

    pub(crate) fn is_passable_at(&self, pos: [f64; 2]) -> bool {
        is_passable_at(&self.board, (self.xs, self.ys), pos)
    }
}

pub(crate) fn is_passable_at(board: &[bool], shape: (usize, usize), pos: [f64; 2]) -> bool {
    let pos = [pos[0] as isize, pos[1] as isize];
    if pos[0] < 0 || shape.0 as isize <= pos[0] || pos[1] < 0 || shape.1 as isize <= pos[1] {
        false
    } else {
        let pos = [pos[0] as usize, pos[1] as usize];
        board[pos[0] + shape.0 * pos[1]]
    }
}
