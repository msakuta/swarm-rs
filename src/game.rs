mod maze;

use cgmath::{InnerSpace, Vector2};

use druid::Data;

use std::{
    cell::RefCell,
    rc::Rc,
    sync::atomic::{AtomicUsize, Ordering},
};

use crate::{
    agent::{Agent, AgentState, Bullet},
    app_data::is_passable_at,
    collision::CollisionShape,
    entity::{Entity, GameEvent},
    measure_time,
    mesh::{create_mesh, Mesh, MeshResult},
    perlin_noise::{gen_terms, perlin_noise_pixel, Xor128},
    qtree::{CellState, QTreeSearcher, Rect},
    spawner::Spawner,
    temp_ents::TempEnt,
    triangle_utils::{check_shape_in_mesh, find_triangle_at},
};

#[derive(Clone, Debug)]
pub(crate) struct Resource {
    pub pos: [f64; 2],
    pub amount: i32,
}

pub(crate) type Board = Vec<bool>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Data)]
pub(crate) enum BoardType {
    Rect,
    Crank,
    Perlin,
    Maze,
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Data)]
pub(crate) enum AvoidanceMode {
    Kinematic,
    Rrt,
    RrtStar,
}

pub(crate) struct BoardParams {
    pub shape: (usize, usize),
    pub seed: u32,
    pub simplify: f64,
    pub maze_expansions: usize,
}

#[derive(Clone, Data)]
pub(crate) struct GameParams {
    pub(crate) avoidance_mode: AvoidanceMode,
    pub(crate) paused: bool,
    pub(crate) avoidance_expands: f64,
    pub(crate) source: Rc<String>,
}

impl GameParams {
    pub fn new() -> Self {
        Self {
            avoidance_mode: AvoidanceMode::RrtStar,
            paused: false,
            avoidance_expands: 1.,
            source: Rc::new("".to_string()),
        }
    }
}

#[derive(Debug)]
pub(crate) struct Game {
    pub(crate) xs: usize,
    pub(crate) ys: usize,
    pub(crate) simplify: f64,
    pub(crate) board: Board,
    pub(crate) mesh: Mesh,
    pub(crate) entities: Vec<RefCell<Entity>>,
    pub(crate) bullets: Vec<Bullet>,
    pub(crate) resources: Vec<Resource>,
    pub(crate) interval: f64,
    pub(crate) rng: Xor128,
    pub(crate) id_gen: usize,
    pub(crate) avoidance_mode: AvoidanceMode,
    pub(crate) avoidance_expands: f64,
    pub(crate) temp_ents: Vec<TempEnt>,
    pub(crate) triangle_profiler: RefCell<Profiler>,
    pub(crate) pixel_profiler: RefCell<Profiler>,
    pub(crate) qtree_profiler: RefCell<Profiler>,
    pub(crate) source: Rc<String>,
    pub(crate) qtree: QTreeSearcher,
}

impl Game {
    pub(crate) fn new() -> Self {
        let seed = 123513;
        let simplify = 1.;

        let xs = 128;
        let ys = 128;

        let MeshResult { board, mesh } = Self::create_perlin_board(&BoardParams {
            shape: (xs, ys),
            seed,
            simplify,
            maze_expansions: 0,
        });

        let id_gen = 0;

        let shape = (xs, ys);
        let (qtree, timer) = measure_time(|| Self::new_qtree(shape, &board, &[]));

        println!("qtree time: {timer:?}");

        Self {
            xs,
            ys,
            simplify,
            board,
            mesh,
            entities: vec![],
            bullets: vec![],
            resources: vec![],
            interval: 32.,
            rng: Xor128::new(9318245),
            id_gen,
            avoidance_mode: AvoidanceMode::RrtStar,
            avoidance_expands: 1.,
            temp_ents: vec![],
            triangle_profiler: RefCell::new(Profiler::new()),
            pixel_profiler: RefCell::new(Profiler::new()),
            qtree_profiler: RefCell::new(Profiler::new()),
            source: Rc::new(String::new()),
            qtree,
        }
    }

    pub fn create_perlin_board(params: &BoardParams) -> MeshResult {
        let shape = params.shape;
        let min_octave = 2;
        let max_octave = 6;
        let mut xor128 = Xor128::new(params.seed);
        let terms = gen_terms(&mut xor128, max_octave);

        create_mesh(shape, params.simplify, |xi, yi| {
            let dx = (xi as isize - shape.0 as isize / 2) as f64;
            let dy = (yi as isize - shape.1 as isize / 2) as f64;
            let noise_val =
                perlin_noise_pixel(xi as f64, yi as f64, min_octave, max_octave, &terms, 0.5);
            0. + (noise_val - 0.5 * (dx * dx + dy * dy).sqrt() / shape.0 as f64) > -0.125
        })
    }

    pub fn create_rect_board(params: &BoardParams) -> MeshResult {
        let (xs, ys) = (params.shape.0 as isize, params.shape.1 as isize);
        create_mesh(params.shape, params.simplify, |xi, yi| {
            let dx = xi as isize - xs / 2;
            let dy = yi as isize - ys / 2;
            dx.abs() < xs / 4 && dy.abs() < ys / 4
        })
    }

    pub fn create_crank_board(params: &BoardParams) -> MeshResult {
        let (xs, ys) = (params.shape.0 as isize, params.shape.1 as isize);
        create_mesh(params.shape, params.simplify, |xi, yi| {
            let dx = xi as isize - xs / 2;
            let dy = yi as isize - ys / 2;
            dx.abs() < xs * 3 / 8
                && dy.abs() < ys / 8
                && !(-xs * 3 / 16 < dx && dx < -xs * 2 / 16 && -ys / 16 < dy)
                && !(xs * 2 / 16 < dx && dx < xs * 3 / 16 && dy < ys / 16)
        })
    }

    pub(crate) fn new_board(&mut self, board_type: BoardType, params: &BoardParams) {
        self.xs = params.shape.0;
        self.ys = params.shape.0;

        let MeshResult { board, mesh } = match board_type {
            BoardType::Rect => Self::create_rect_board(&params),
            BoardType::Crank => Self::create_crank_board(&params),
            BoardType::Perlin => Self::create_perlin_board(&params),
            BoardType::Maze => Self::create_maze_board(&params),
        };

        self.qtree = Self::new_qtree(params.shape, &board, &[]);
        self.board = board;
        self.mesh = mesh;
        self.entities = vec![];
        self.bullets = vec![];
    }

    fn new_qtree(
        shape: (usize, usize),
        board: &Board,
        entities: &[RefCell<Entity>],
    ) -> QTreeSearcher {
        let mut qtree = QTreeSearcher::new();
        let calls: AtomicUsize = AtomicUsize::new(0);
        let unpassables: AtomicUsize = AtomicUsize::new(0);
        let shapes: Vec<_> = entities
            .iter()
            .map(|entity| {
                let entity = entity.borrow();
                (entity.get_id(), entity.get_shape().to_aabb())
            })
            .collect();
        qtree.initialize(shape, &|rect: Rect| {
            let mut has_passable = false;
            let mut has_unpassable = None;
            for x in rect[0]..rect[2] {
                for y in rect[1]..rect[3] {
                    calls.fetch_add(1, Ordering::Relaxed);
                    let mut has_unpassable_local = None;
                    if !is_passable_at(board, shape, [x as f64 + 0.5, y as f64 + 0.5]) {
                        unpassables.fetch_add(1, Ordering::Relaxed);
                        has_unpassable_local = Some(CellState::Obstacle);
                    } else {
                        let (fx, fy) = (x as f64, y as f64);
                        for (id, aabb) in &shapes {
                            if aabb[0].floor() <= fx
                                && fx <= aabb[2].ceil()
                                && aabb[1].floor() <= fy
                                && fy <= aabb[3].ceil()
                            {
                                has_unpassable_local = Some(CellState::Occupied(*id));
                                break;
                            }
                            // for y in bbox.center.y - bbox.ys..bbox.center.y + bbox.ys {
                            //     for x in bbox.center.y - bbox.xs..bbox.center.x + bbox.xs {

                            //     }
                            // }
                        }
                    }
                    if has_unpassable_local.is_none() {
                        has_passable = true;
                    }
                    if has_unpassable.is_none() {
                        has_unpassable = has_unpassable_local;
                    }
                    if has_passable && has_unpassable.is_some() {
                        return CellState::Mixed;
                    }
                }
            }
            if has_passable {
                CellState::Free
            } else if let Some(state) = has_unpassable {
                state
            } else {
                CellState::Obstacle
            }
        });
        println!("calls: {:?} unpassables: {unpassables:?}", calls);
        qtree
    }

    pub(crate) fn try_new_agent(
        &mut self,
        pos: [f64; 2],
        team: usize,
        entities: &[RefCell<Entity>],
        static_: bool,
        randomness: f64,
    ) -> Option<Entity> {
        const STATIC_SOURCE_FILE: &str = include_str!("../test_obstacle.txt");
        let rng = &mut self.rng;
        let id_gen = &mut self.id_gen;
        let triangle_labels = &self.mesh.triangle_labels;
        let largest_label = self.mesh.largest_label;

        for _ in 0..10 {
            let state_candidate = AgentState {
                x: pos[0] + (rng.next() - 0.5) * randomness,
                y: pos[1] + (rng.next() - 0.5) * randomness,
                heading: rng.next() * std::f64::consts::PI * 2.,
            };

            if Agent::qtree_collision(None, state_candidate, entities) {
                continue;
            }

            if Agent::collision_check(None, state_candidate, entities, false) {
                continue;
            }

            if !is_passable_at(
                &self.board,
                (self.xs, self.ys),
                [state_candidate.x, state_candidate.y],
            ) {
                continue;
            }

            if let Some(tri) = find_triangle_at(
                &self.mesh,
                state_candidate.into(),
                &mut self.triangle_profiler.borrow_mut(),
            ) {
                if Some(triangle_labels[tri]) == largest_label {
                    let agent = Agent::new(
                        id_gen,
                        state_candidate.into(),
                        state_candidate.heading,
                        team,
                        if static_ {
                            STATIC_SOURCE_FILE
                        } else {
                            &self.source
                        },
                    );
                    match agent {
                        Ok(agent) => return Some(Entity::Agent(agent)),
                        Err(e) => println!("Failed to create an Agent! {e}"),
                    }
                }
            } else {
                println!("Triangle not fonud! {pos:?}");
            }
        }
        None
    }

    /// Check collision with the environment
    pub(crate) fn check_hit(&self, state: &CollisionShape) -> bool {
        // let triangle_labels = &self.mesh.triangle_labels;
        // let largest_label = self.mesh.largest_label;
        // if let Some(tri) =
        //     find_triangle_at(&self.mesh, pos, &mut *self.triangle_profiler.borrow_mut())
        // {
        //     if Some(triangle_labels[tri]) == largest_label {
        //         return true;
        //     }
        // }
        check_shape_in_mesh(&self.mesh, &state, &mut self.triangle_profiler.borrow_mut())
    }

    fn try_new_spawner(&mut self, team: usize) -> Option<Entity> {
        for _ in 0..10 {
            let rng = &mut self.rng;
            let pos_candidate = [rng.next() * self.xs as f64, rng.next() * self.ys as f64];
            if let Some(tri) = find_triangle_at(
                &self.mesh,
                pos_candidate,
                &mut self.triangle_profiler.borrow_mut(),
            ) {
                if Some(self.mesh.triangle_labels[tri]) == self.mesh.largest_label {
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

    fn try_new_resource(&mut self) {
        self.resources = std::mem::take(&mut self.resources)
            .into_iter()
            .filter(|res| 0 < res.amount)
            .collect();
        if 10 < self.resources.len() {
            return;
        }
        for _ in 0..10 {
            let rng = &mut self.rng;
            let pos_candidate = [rng.next() * self.xs as f64, rng.next() * self.ys as f64];
            if !is_passable_at(&self.board, (self.xs, self.ys), pos_candidate) {
                continue;
            }

            if let Some(tri) = find_triangle_at(
                &self.mesh,
                pos_candidate,
                &mut self.triangle_profiler.borrow_mut(),
            ) {
                if Some(self.mesh.triangle_labels[tri]) == self.mesh.largest_label {
                    if self.board[pos_candidate[0] as usize + self.xs * pos_candidate[1] as usize] {
                        self.resources.push(Resource {
                            pos: pos_candidate,
                            amount: (rng.nexti() % 128 + 10) as i32,
                        });
                    }
                }
            }
        }
    }

    pub(crate) fn set_params(&mut self, params: &GameParams) {
        self.avoidance_mode = params.avoidance_mode;
        self.avoidance_expands = params.avoidance_expands;
        self.source = params.source.clone();
    }

    pub(crate) fn update(&mut self) {
        let mut entities = std::mem::take(&mut self.entities);
        let mut bullets = std::mem::take(&mut self.bullets);
        let mut events = vec![];
        for entity in entities.iter() {
            let mut entity = entity.borrow_mut();
            events.extend(entity.update(self, &entities, &mut bullets));
        }

        for event in events {
            match event {
                GameEvent::SpawnAgent { pos, team, spawner } => {
                    if let Some(agent) = self.try_new_agent(pos, team, &entities, false, 10.) {
                        entities.push(RefCell::new(agent));
                        if let Some(spawner) = entities
                            .iter_mut()
                            .find(|ent| ent.borrow().get_id() == spawner)
                        {
                            spawner.borrow_mut().remove_resource(100);
                        }
                    }
                }
            }
        }

        // let (qtree, timer) =
        //     measure_time(|| Rc::new(Self::new_qtree((self.xs, self.ys), &self.board, &entities)));
        // self.qtree = qtree;

        self.entities = entities;
        self.bullets = bullets;

        {
            let agents = &self.entities;
            let mut temp_ents = std::mem::take(&mut self.temp_ents);
            self.bullets = self
                .bullets
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
                        if let Some(agent_vertices) = agent.get_shape().to_vertices() {
                            if separating_axis(
                                &Vector2::from(bullet.pos),
                                &Vector2::from(bullet.velo),
                                agent_vertices.into_iter().map(Vector2::from),
                            ) {
                                temp_ents.push(TempEnt::new(bullet.pos));
                                if agent.damage() {
                                    agent.set_active(false);
                                }
                                println!("Agent {} is being killed", agent.get_id());
                                return None;
                            }
                        }
                    }
                    let mut ret = bullet.clone();
                    ret.pos = newpos;
                    ret.traveled += Vector2::from(bullet.velo).magnitude();
                    Some(ret)
                })
                .collect();

            self.temp_ents = temp_ents
                .into_iter()
                .filter_map(|mut ent| if ent.update() { Some(ent) } else { None })
                .collect();
        }

        let (_, timer) = measure_time(|| {
            let qtree = &mut self.qtree;
            let entities = &self.entities;

            qtree.start_update();

            fn update_aabb(
                qtree: &mut QTreeSearcher,
                aabb: [f64; 4],
                cell_state: impl Fn([i32; 2]) -> CellState,
            ) {
                for y in aabb[1].floor() as i32..aabb[3].ceil() as i32 {
                    for x in aabb[0].floor() as i32..aabb[2].ceil() as i32 {
                        if let Err(e) = qtree.update([x, y], cell_state([x, y])) {
                            println!("qtree.update error: {e}");
                        }
                    }
                }
            }

            let get_background = |pos: [i32; 2]| {
                if is_passable_at(
                    &self.board,
                    (self.xs, self.ys),
                    [pos[0] as f64 + 0.5, pos[1] as f64 + 0.5],
                ) {
                    CellState::Free
                } else {
                    CellState::Obstacle
                }
            };

            // Clear the previous cells
            for shape in entities
                .iter()
                .filter_map(|entity| entity.borrow().get_last_state())
            {
                update_aabb(qtree, shape.to_aabb(), get_background);
            }

            // Update cells of live entities and clear the cells of dead ones
            for entity in entities.iter() {
                let entity = entity.borrow();
                let id = entity.get_id();
                let aabb = entity.get_shape().to_aabb();
                update_aabb(qtree, aabb, |pos| {
                    if let CellState::Obstacle = get_background(pos) {
                        CellState::Obstacle
                    } else if entity.get_active() {
                        CellState::Occupied(id)
                    } else {
                        get_background(pos)
                    }
                });
            }

            qtree.finish_update();
        });

        self.qtree_profiler.borrow_mut().add(timer);

        let mut entities: Vec<_> = std::mem::take(&mut self.entities)
            .into_iter()
            .filter(|agent| agent.borrow().get_active())
            .collect();

        // if entities.is_empty() {
        //     println!("Adding agents");
        //     let pos = [self.xs as f64 * 2. / 8., self.ys as f64 * 9. / 16.];
        //     if let Some(agent) = self.try_new_agent(pos, 0, &entities, false, 0.) {
        //         entities.push(RefCell::new(agent));
        //     }
        //     let pos = [self.xs as f64 / 2., self.ys as f64 / 2.];
        //     if let Some(agent) = self.try_new_agent(pos, 0, &entities, true, 0.) {
        //         entities.push(RefCell::new(agent));
        //     }
        // }

        for team in 0..2 {
            let rng = &mut self.rng;
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
        self.entities = entities;

        self.try_new_resource();
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

/// Separating axis theorem is relatively fast algorithm to detect collision between convex polygons,
/// but it can only predict linear motions.
pub(crate) fn separating_axis(
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
