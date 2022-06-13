use crate::perlin_noise::{gen_terms, perlin_noise_pixel, Xor128};
use druid::{Data, Lens, Vec2};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

#[allow(dead_code)]
pub(crate) type Polygon = geo::Polygon<f64>;

#[derive(Clone, PartialEq, Eq, Data)]
pub(crate) enum LineMode {
    Line,
    Polygon,
}

#[derive(Clone, Lens, Data)]
pub(crate) struct AppData {
    pub(crate) rows_text: String,
    pub(crate) columns_text: String,
    pub(crate) width_text: String,
    pub(crate) vertex_edit: bool,
    pub(crate) group_edit: bool,
    pub(crate) group_radius_text: String,
    pub(crate) xs: usize,
    pub(crate) ys: usize,
    pub(crate) board: Rc<Vec<bool>>,
    pub(crate) line_mode: LineMode,
    pub(crate) origin: Vec2,
    pub(crate) scale: f64,
    pub(crate) message: String,
    pub(crate) get_mesh_time: f64,
    #[data(ignore)]
    pub(crate) render_mesh_time: Cell<f64>,
    pub(crate) render_stats: Rc<RefCell<String>>,
}

impl AppData {
    pub(crate) fn new() -> Self {
        let rows = 8;
        let columns = 8;
        let width = 100.;
        let group_radius = 100.;

        let xs = 128;
        let ys = 128;

        let bits = 6;
        let mut xor128 = Xor128::new(123513);
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

        Self {
            rows_text: rows.to_string(),
            columns_text: columns.to_string(),
            width_text: width.to_string(),
            vertex_edit: true,
            group_edit: true,
            group_radius_text: group_radius.to_string(),
            xs,
            ys,
            board: Rc::new(board),
            line_mode: LineMode::Line,
            origin: Vec2::new(400., 400.),
            scale: 1.,
            message: "".to_string(),
            render_mesh_time: Cell::new(0.),
            get_mesh_time: 0.,
            render_stats: Rc::new(RefCell::new("".to_string())),
        }
    }
}
