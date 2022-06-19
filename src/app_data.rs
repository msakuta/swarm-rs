use crate::{
    agent::{Agent, Bullet},
    marching_squares::{trace_lines, BoolField},
    perlin_noise::{gen_terms, perlin_noise_pixel, Xor128},
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
    pub(crate) triangulation_visible: bool,
    pub(crate) origin: Vec2,
    pub(crate) scale: f64,
    pub(crate) message: String,
    pub(crate) get_board_time: f64,
    #[data(ignore)]
    pub(crate) render_board_time: Cell<f64>,
    pub(crate) render_stats: Rc<RefCell<String>>,
    pub(crate) agents: Rc<Vec<Agent>>,
    pub(crate) bullets: Rc<Vec<Bullet>>,
    pub(crate) paused: bool,
    pub(crate) interval: f64,
}

impl AppData {
    pub(crate) fn new() -> Self {
        let seed = 123513;
        let simplify = 1.;

        let xs = 128;
        let ys = 128;

        let (board, simplified_border, points) = AppData::create_board((xs, ys), seed, simplify);

        let mut id_gen = 0;
        let mut agents = vec![];
        let mut agent_rng = Xor128::new(seed);
        for i in 0..4 {
            for _ in 0..10 {
                let pos_candidate = [agent_rng.next() * xs as f64, agent_rng.next() * ys as f64];
                if board[pos_candidate[0] as usize + xs * pos_candidate[1] as usize] {
                    agents.push(Agent::new(&mut id_gen, pos_candidate, i % 2));
                    break;
                }
            }
        }

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
            simplified_visible: true,
            triangulation: Rc::new(triangulate(&points)),
            points: Rc::new(points),
            triangulation_visible: true,
            origin: Vec2::new(400., 400.),
            scale: 1.,
            message: "".to_string(),
            render_board_time: Cell::new(0.),
            get_board_time: 0.,
            render_stats: Rc::new(RefCell::new("".to_string())),
            agents: Rc::new(agents),
            bullets: Rc::new(vec![]),
            paused: false,
            interval: 100.,
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

    pub(crate) fn update(&mut self) {
        let agents = Rc::make_mut(&mut self.agents);
        for i in 0..agents.len() {
            let (first, mid) = agents.split_at_mut(i);
            let (agent, last) = mid.split_first_mut().unwrap();
            let rest = || first.iter().chain(last.iter());
            agent.find_enemy(rest());
            if let Some(target) = agent
                .target
                .and_then(|target| rest().find(|a| a.id == target))
            {
                if 10. < Vector2::from(target.pos).distance(Vector2::from(agent.pos)) {
                    agent.move_to(
                        self.board.as_ref(),
                        (self.xs as isize, self.ys as isize),
                        target.pos,
                    );
                }
                agent.shoot_bullet(Rc::make_mut(&mut self.bullets), target.pos);
            }
            agent.update();
        }

        let bullets = Rc::make_mut(&mut self.bullets);
        for bullet in bullets {
            bullet.pos = (Vector2::from(bullet.pos) + Vector2::from(bullet.velo)).into();
        }

        let bullets = std::mem::take(Rc::make_mut(&mut self.bullets));
        *Rc::make_mut(&mut self.bullets) = bullets
            .iter()
            .filter(|bullet| self.is_passable_at(bullet.pos))
            .map(|bullet| bullet.clone())
            .collect();
    }

    pub(crate) fn is_passable_at(&self, pos: [f64; 2]) -> bool {
        let pos = [pos[0] as isize, pos[1] as isize];
        if pos[0] < 0 || self.xs as isize <= pos[0] || pos[1] < 0 || self.ys as isize <= pos[1] {
            false
        } else {
            let pos = [pos[0] as usize, pos[1] as usize];
            self.board[pos[0] + self.xs * pos[1]]
        }
    }
}
