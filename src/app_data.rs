use crate::{
    agent::{Agent, Bullet},
    marching_squares::{trace_lines, BoolField},
    perlin_noise::{gen_terms, perlin_noise_pixel, Xor128},
    triangle_utils::{center_of_triangle_obj, label_triangles},
    WINDOW_HEIGHT,
};
use ::cgmath::{MetricSpace, Vector2};
use ::delaunator::{triangulate, Triangulation};
use druid::{piet::kurbo::BezPath, Data, Lens, Point, Vec2};
use std::{
    cell::{Cell, RefCell},
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
    pub(crate) board: Rc<Vec<bool>>,
    pub(crate) line_mode: LineMode,
    pub(crate) simplified_border: Rc<Vec<BezPath>>,
    pub(crate) simplified_visible: bool,
    pub(crate) triangulation: Rc<Triangulation>,
    pub(crate) points: Rc<Vec<delaunator::Point>>,
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
    pub(crate) agents: Rc<Vec<RefCell<Agent>>>,
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

        let mut id_gen = 0;
        let mut agents = vec![];
        let mut agent_rng = Xor128::new(seed);
        for i in 0..4 {
            if let Some(agent) =
                Self::try_new_agent(&mut agent_rng, &mut id_gen, &board, (xs, ys), i % 2)
            {
                agents.push(RefCell::new(agent));
            }
        }

        let triangulation = triangulate(&points);

        let triangle_passable =
            Self::calc_passable_triangles(&board, (xs, ys), &points, &triangulation);

        let triangle_labels = label_triangles(&triangulation, &triangle_passable);

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
            agents: Rc::new(agents),
            bullets: Rc::new(vec![]),
            paused: false,
            interval: 100.,
            rng: Rc::new(agent_rng),
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

        self.board = Rc::new(board);
        self.simplified_border = Rc::new(simplified_border);
        self.triangulation = Rc::new(triangulation);
        self.points = Rc::new(points);
        self.triangle_passable = Rc::new(triangle_passable);
        self.triangle_labels = Rc::new(triangle_labels);
        self.agents = Rc::new(vec![]);
        self.bullets = Rc::new(vec![]);
    }

    fn try_new_agent(
        rng: &mut Xor128,
        id_gen: &mut usize,
        board: &Board,
        (xs, ys): (usize, usize),
        team: usize,
    ) -> Option<Agent> {
        for _ in 0..10 {
            let pos_candidate = [rng.next() * xs as f64, rng.next() * ys as f64];
            if board[pos_candidate[0] as usize + xs * pos_candidate[1] as usize] {
                return Some(Agent::new(id_gen, pos_candidate, team));
            }
        }
        None
    }

    pub(crate) fn update(&mut self) {
        let agents = &self.agents;
        for agent in agents.iter() {
            let mut agent = agent.borrow_mut();
            agent.find_enemy(agents);
            agent.update(
                agents,
                &self.triangulation,
                &self.points,
                &self.triangle_passable,
                &self.board,
                (self.xs as isize, self.ys as isize),
                Rc::make_mut(&mut self.bullets),
            );
        }

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
                        if agent.team == bullet.team {
                            continue;
                        }
                        let dist2 = Vector2::from(agent.pos).distance2(Vector2::from(newpos));
                        if dist2 < 3. * 3. {
                            agent.active = false;
                            println!("Agent {} is being killed", agent.id);
                            return None;
                        }
                    }
                    let mut ret = bullet.clone();
                    ret.pos = newpos;
                    Some(ret)
                })
                .collect(),
        );

        let mut agents: Vec<_> = self
            .agents
            .iter()
            .filter(|agent| agent.borrow().active)
            .map(|agent| agent.clone())
            .collect();

        let rng = Rc::make_mut(&mut self.rng);
        for team in 0..2 {
            if agents
                .iter()
                .filter(|agent| agent.borrow().team == team)
                .count()
                < 5
                && rng.next() < 0.1
            {
                if let Some(agent) = Self::try_new_agent(
                    rng,
                    &mut self.id_gen,
                    &self.board,
                    (self.xs, self.ys),
                    team,
                ) {
                    agents.push(RefCell::new(agent));
                }
            }
        }
        self.agents = Rc::new(agents);
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
