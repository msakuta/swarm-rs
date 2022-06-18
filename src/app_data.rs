use crate::{
    marching_squares::{trace_lines, BoolField},
    perlin_noise::{gen_terms, perlin_noise_pixel, Xor128},
};
use druid::{piet::kurbo::BezPath, Data, Lens, Point, Vec2};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

#[derive(Clone, PartialEq, Eq, Data)]
pub(crate) enum LineMode {
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
    pub(crate) origin: Vec2,
    pub(crate) scale: f64,
    pub(crate) message: String,
    pub(crate) get_board_time: f64,
    #[data(ignore)]
    pub(crate) render_board_time: Cell<f64>,
    pub(crate) render_stats: Rc<RefCell<String>>,
}

impl AppData {
    pub(crate) fn new() -> Self {
        let seed = 123513;
        let simplify = 1.;

        let xs = 128;
        let ys = 128;

        let (board, simplified_border) = AppData::create_board((xs, ys), seed, simplify);

        Self {
            rows_text: xs.to_string(),
            columns_text: ys.to_string(),
            seed_text: seed.to_string(),
            simplify_text: simplify.to_string(),
            xs,
            ys,
            board: Rc::new(board),
            line_mode: LineMode::Line,
            simplified_border: Rc::new(simplified_border),
            origin: Vec2::new(400., 400.),
            scale: 1.,
            message: "".to_string(),
            render_board_time: Cell::new(0.),
            get_board_time: 0.,
            render_stats: Rc::new(RefCell::new("".to_string())),
        }
    }

    pub fn create_board(
        (xs, ys): (usize, usize),
        seed: u32,
        simplify: f64,
    ) -> (Vec<bool>, Vec<BezPath>) {
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

        let to_point = |p: [f64; 2]| Point::new(p[0] as f64 + 1., p[1] as f64 + 1.);

        let lines = trace_lines(&field);
        let mut simplified_vertices = 0;
        for line in &lines {
            let simplified = if simplify == 0. {
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
                    simplify,
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

        (board, simplified_border)
    }
}
