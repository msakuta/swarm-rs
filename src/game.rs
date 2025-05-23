mod iterative_maze;
mod maze;
mod rooms;

use cgmath::{InnerSpace, Vector2};

use std::{
    cell::RefCell,
    collections::HashMap,
    rc::Rc,
    sync::atomic::{AtomicUsize, Ordering},
};

use crate::{
    agent::{Agent, AgentClass, AgentState, Bullet},
    collision::CollisionShape,
    entity::{Entity, GameEvent, VISION_RANGE},
    fog_of_war::{precompute_raycast_map, FogOfWar, FogRaycastMap, FOG_MAX_AGE},
    measure_time,
    mesh::{create_mesh, Mesh, MeshResult},
    perlin_noise::{gen_terms, perlin_noise_pixel, Xor128},
    qtree::{CellState, QTreeSearcher, Rect},
    spawner::Spawner,
    temp_ents::TempEnt,
    triangle_utils::check_shape_in_mesh,
};

#[cfg(feature = "druid")]
use druid::Data;

#[derive(Clone, Debug)]
pub struct Resource {
    pub pos: [f64; 2],
    pub amount: i32,
}

pub(crate) type Board = Vec<bool>;

#[cfg_attr(feature = "druid", derive(Data))]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoardType {
    Rect,
    Crank,
    Perlin,
    Rooms,
    Maze,
    IterativeMaze,
}

#[derive(Debug, Clone)]
pub struct Profiler {
    total: f64,
    count: usize,
}

pub enum UpdateResult {
    Running,
    TeamWon(usize),
}

impl Profiler {
    pub(crate) fn new() -> Self {
        Self {
            total: 0.,
            count: 0,
        }
    }

    pub fn get_average(&self) -> f64 {
        if self.count == 0 {
            0.
        } else {
            self.total / self.count as f64
        }
    }

    pub fn get_count(&self) -> usize {
        self.count
    }

    pub(crate) fn add(&mut self, sample: f64) {
        self.total += sample;
        self.count += 1;
    }
}

#[cfg_attr(feature = "druid", derive(Data))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AvoidanceMode {
    Kinematic,
    Rrt,
    RrtStar,
}

pub struct BoardParams {
    pub shape: (usize, usize),
    pub seed: u32,
    pub simplify: f64,
    pub maze_expansions: usize,
}

#[cfg_attr(feature = "druid", derive(Data))]
#[derive(Clone, Debug, Default)]
pub struct TeamConfig {
    pub agent_source: Rc<String>,
    pub spawner_source: Rc<String>,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct TeamStats {
    pub spawned: usize,
    pub kills: usize,
    pub wins: usize,
}

#[cfg_attr(feature = "druid", derive(Data))]
#[derive(Clone, Debug)]
pub struct GameParams {
    pub avoidance_mode: AvoidanceMode,
    pub paused: bool,
    pub avoidance_expands: f64,
    pub agent_count: usize,
    /// Fog of War, some area of the map is covered by lack of knowledge, adding some depth to the strategy.
    pub fow: bool,
    /// Use raycasting to check visibility to clear fog of war. It can be expensive.
    pub fow_raycasting: bool,
    pub fow_raycast_visible: bool,
    pub teams: [TeamConfig; 2],
}

impl GameParams {
    pub fn new() -> Self {
        Self {
            avoidance_mode: AvoidanceMode::RrtStar,
            paused: false,
            avoidance_expands: 1.,
            agent_count: 3,
            fow: true,
            fow_raycasting: true,
            fow_raycast_visible: false,
            teams: Default::default(),
        }
    }
}

#[derive(Debug)]
pub struct Game {
    pub(crate) xs: usize,
    pub(crate) ys: usize,
    pub simplify: f64,
    pub board: Board,
    pub mesh: Mesh,
    pub entities: Vec<RefCell<Entity>>,
    pub bullets: Vec<Bullet>,
    pub resources: Vec<Resource>,
    pub interval: f64,
    /// The last updated age of each pixel. Newest updated pixels should have self.global_time.
    pub fog: [FogOfWar; 2],
    pub(crate) rng: Xor128,
    pub(crate) id_gen: usize,
    pub temp_ents: Vec<TempEnt>,
    pub triangle_profiler: RefCell<Profiler>,
    pub pixel_profiler: RefCell<Profiler>,
    pub qtree_profiler: RefCell<Profiler>,
    pub path_find_profiler: RefCell<Profiler>,
    pub fow_raycast_profiler: RefCell<Profiler>,
    pub params: GameParams,
    pub stats: [TeamStats; 2],
    pub global_time: i32,
    pub qtree: QTreeSearcher,

    pub enable_raycast_board: bool,
    /// A visualization of visited pixels by raycasting visibility checking
    pub raycast_board: RefCell<Vec<u8>>,
    pub fog_rays: Vec<Vec<[i32; 2]>>,
    pub fog_raycast_map: FogRaycastMap,
    pub(crate) fog_raycast_map_forward: FogRaycastMap,
    /// Raycast maps for debug visualization
    pub fog_raycast_map_real: Vec<Vec<[[i32; 2]; 2]>>,
    /// Cached raycast maps for each Entity
    pub(crate) fog_raycast_map_cache: HashMap<usize, ([i32; 2], Vec<bool>)>,
}

impl Game {
    pub fn new() -> Self {
        let seed = 123513;
        let simplify = 1.;

        let xs = 128;
        let ys = 128;

        let MeshResult { board, mesh } = Self::create_rooms_board(&BoardParams {
            shape: (xs, ys),
            seed,
            simplify,
            maze_expansions: 0,
        });

        let id_gen = 0;

        let shape = (xs, ys);
        let (qtree, timer) = measure_time(|| Self::new_qtree(shape, &board, &[]));

        let fog = FogOfWar::new(&board);
        let fog = [fog.clone(), fog];

        println!("qtree time: {timer:?}");

        let (fog_raycast_map, fog_raycast_map_forward) =
            precompute_raycast_map(VISION_RANGE as usize);

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
            fog,
            rng: Xor128::new(9318245),
            id_gen,
            temp_ents: vec![],
            triangle_profiler: RefCell::new(Profiler::new()),
            pixel_profiler: RefCell::new(Profiler::new()),
            qtree_profiler: RefCell::new(Profiler::new()),
            path_find_profiler: RefCell::new(Profiler::new()),
            fow_raycast_profiler: RefCell::new(Profiler::new()),
            params: GameParams::new(),
            stats: Default::default(),
            global_time: 0,
            qtree,
            enable_raycast_board: false,
            raycast_board: RefCell::new(vec![]),
            fog_rays: vec![],
            fog_raycast_map,
            fog_raycast_map_forward,
            fog_raycast_map_real: vec![],
            fog_raycast_map_cache: HashMap::new(),
        }
    }

    pub fn shape(&self) -> (usize, usize) {
        (self.xs, self.ys)
    }

    pub fn init(&mut self) {
        for team in 0..2 {
            if !self
                .entities
                .iter()
                .any(|agent| !agent.borrow().is_agent() && agent.borrow().get_team() == team)
            {
                let spawner = self.try_new_spawner(team);
                if let Some(spawner) = spawner {
                    self.entities.push(RefCell::new(spawner));
                }
            }
        }
    }

    pub(crate) fn create_perlin_board(params: &BoardParams) -> MeshResult {
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

    pub(crate) fn create_rect_board(params: &BoardParams) -> MeshResult {
        let (xs, ys) = (params.shape.0 as isize, params.shape.1 as isize);
        create_mesh(params.shape, params.simplify, |xi, yi| {
            let dx = xi as isize - xs / 2;
            let dy = yi as isize - ys / 2;
            dx.abs() < xs / 4 && dy.abs() < ys / 4
        })
    }

    pub(crate) fn create_crank_board(params: &BoardParams) -> MeshResult {
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
    pub fn new_board(&mut self, board_type: BoardType, params: &BoardParams) {
        self.xs = params.shape.0;
        self.ys = params.shape.1;

        let MeshResult { board, mesh } = match board_type {
            BoardType::Rect => Self::create_rect_board(&params),
            BoardType::Crank => Self::create_crank_board(&params),
            BoardType::Perlin => Self::create_perlin_board(&params),
            BoardType::Rooms => Self::create_rooms_board(&params),
            BoardType::Maze => Self::create_maze_board(&params),
            BoardType::IterativeMaze => Self::create_iterative_maze_board(&params),
        };

        let fog = FogOfWar::new(&board);

        self.qtree = Self::new_qtree(params.shape, &board, &[]);
        self.raycast_board = RefCell::new(vec![]);
        self.board = board;
        self.fog = [fog.clone(), fog];
        self.mesh = mesh;
        self.entities = vec![];
        self.bullets = vec![];
        self.resources.clear();
        self.global_time = 0;
        self.fog_raycast_map_cache.clear();
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
        let init_result = qtree.initialize(shape, &|rect: Rect| {
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
        match init_result {
            Ok(_) => println!("calls: {:?} unpassables: {unpassables:?}", calls),
            Err(e) => println!("Failed to initialize QTree: {e}"),
        }
        qtree
    }

    pub(crate) fn try_new_agent(
        &mut self,
        pos: [f64; 2],
        team: usize,
        class: AgentClass,
        entities: &[RefCell<Entity>],
        static_: bool,
        randomness: f64,
    ) -> Option<Entity> {
        const STATIC_SOURCE_FILE: &str = include_str!("../behavior_tree_config/test_obstacle.btc");
        let rng = &mut self.rng;
        let id_gen = &mut self.id_gen;
        // let triangle_labels = &self.mesh.triangle_labels;
        // let largest_label = self.mesh.largest_label;

        for _ in 0..10 {
            let state_candidate = AgentState {
                x: pos[0] + (rng.next() - 0.5) * randomness,
                y: pos[1] + (rng.next() - 0.5) * randomness,
                heading: rng.next() * std::f64::consts::PI * 2.,
            };

            if Agent::qtree_collision(None, state_candidate, class, entities) {
                continue;
            }

            if Agent::collision_check(None, state_candidate, class, entities, false) {
                continue;
            }

            if !is_passable_at(
                &self.board,
                (self.xs, self.ys),
                [state_candidate.x, state_candidate.y],
            ) {
                continue;
            }

            let agent = Agent::new(
                id_gen,
                state_candidate.into(),
                state_candidate.heading,
                team,
                class,
                if static_ {
                    Rc::new(STATIC_SOURCE_FILE.to_string())
                } else {
                    self.params.teams[team].agent_source.clone()
                },
            );
            match agent {
                Ok(agent) => return Some(Entity::Agent(agent)),
                Err(e) => println!("Failed to create an Agent! {e}"),
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
            if self
                .qtree
                .check_collision(&Spawner::collision_shape(pos_candidate).to_aabb())
            {
                continue;
            }
            if Spawner::qtree_collision(None, pos_candidate, &self.entities) {
                continue;
            }
            if self.board[pos_candidate[0] as usize + self.xs * pos_candidate[1] as usize] {
                let spawner = Spawner::new(
                    &mut self.id_gen,
                    pos_candidate,
                    team,
                    self.params.teams[team].spawner_source.clone(),
                );
                match spawner {
                    Ok(spawner) => return Some(Entity::Spawner(spawner)),
                    Err(err) => println!("Spawner failed to create!: {err}"),
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

            if !matches!(self.qtree.find(pos_candidate), Some((_, CellState::Free))) {
                continue;
            }

            if self.board[pos_candidate[0] as usize + self.xs * pos_candidate[1] as usize] {
                self.resources.push(Resource {
                    pos: pos_candidate,
                    amount: (rng.nexti() % 128 + 80) as i32,
                });
            }
        }
    }

    pub fn set_params(&mut self, params: &GameParams) {
        self.params = params.clone();
    }

    pub fn update(&mut self) -> UpdateResult {
        self.global_time += 1;

        self.fog_rays.clear();
        self.fog_raycast_map_real.clear();

        if self.enable_raycast_board {
            let mut raycast_board = self.raycast_board.borrow_mut();
            if raycast_board.len() != self.board.len() {
                raycast_board.resize(self.board.len(), 0);
            }
            raycast_board.fill(0);
        }

        let mut entities = std::mem::take(&mut self.entities);
        let mut bullets = std::mem::take(&mut self.bullets);
        let mut events = vec![];
        for entity in entities.iter() {
            let mut entity = entity.borrow_mut();
            events.extend(entity.update(self, &entities, &mut bullets));
        }

        for event in events {
            match event {
                GameEvent::SpawnAgent {
                    pos,
                    team,
                    class,
                    spawner,
                } => {
                    if let Some(agent) = self.try_new_agent(pos, team, class, &entities, false, 10.)
                    {
                        println!("Spawning agent {class:?}");
                        entities.push(RefCell::new(agent));
                        self.stats[team].spawned += 1;
                        if let Some(spawner) = entities
                            .iter_mut()
                            .find(|ent| ent.borrow().get_id() == spawner)
                        {
                            spawner.borrow_mut().remove_resource(class.cost());
                        }
                    }
                }
            }
        }

        // let (qtree, timer) =
        //     measure_time(|| Rc::new(Self::new_qtree((self.xs, self.ys), &self.board, &entities)));
        // self.qtree = qtree;

        self.entities = entities;
        // self.bullets = bullets;

        {
            let agents = &self.entities;
            let mut temp_ents = std::mem::take(&mut self.temp_ents);
            let mut kills = [0usize; 2];
            bullets.retain_mut(|bullet| {
                if !self.is_passable_at(bullet.pos) {
                    return false;
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
                            let temp_ent = match bullet.shooter_class {
                                AgentClass::Worker => {
                                    TempEnt::new(bullet.pos, crate::temp_ents::MAX_TTL / 2., 1.)
                                }
                                AgentClass::Fighter => {
                                    TempEnt::new(bullet.pos, crate::temp_ents::MAX_TTL, 2.)
                                }
                            };
                            temp_ents.push(temp_ent);
                            if agent.damage(bullet.damage) {
                                agent.set_active(false);
                                kills[bullet.team] += 1;
                                println!("Entity {} is being killed", agent.get_id());
                            }
                            return false;
                        }
                    }
                }
                bullet.pos = newpos;
                bullet.traveled += Vector2::from(bullet.velo).magnitude();
                true
            });
            self.bullets = bullets;

            self.temp_ents.retain_mut(|ent| ent.update());

            for team in 0..self.stats.len() {
                self.stats[team].kills += kills[team];
            }
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

        self.entities.retain(|agent| agent.borrow().get_active());
        let entities = std::mem::take(&mut self.entities);

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
            self.fog_resource(team);
            self.fog_entities(team, &entities);

            if !entities
                .iter()
                .any(|agent| !agent.borrow().is_agent() && agent.borrow().get_team() == team)
            {
                self.entities = entities;
                let won_team = (team + 1) % 2;
                self.stats[won_team].wins += 1;
                return UpdateResult::TeamWon(won_team);
            }
        }
        self.entities = entities;

        self.try_new_resource();

        UpdateResult::Running
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

    pub fn is_clear_fog_at(&self, team: usize, pos: [f64; 2]) -> bool {
        if !self.params.fow {
            return true;
        }
        if pos[0] < 0. || self.xs <= pos[0] as usize || pos[1] < 0. || self.ys <= pos[1] as usize {
            false
        } else {
            self.global_time <= self.fog[team].fow[pos[0] as usize + pos[1] as usize * self.xs]
        }
    }

    pub(crate) fn is_fog_older_than(&self, team: usize, pos: [f64; 2], age: i32) -> bool {
        if !self.params.fow {
            return true;
        }
        if pos[0] < 0. || self.xs <= pos[0] as usize || pos[1] < 0. || self.ys <= pos[1] as usize {
            false
        } else {
            age <= self
                .global_time
                .saturating_sub(self.fog[team].fow[pos[0] as usize + pos[1] as usize * self.xs])
        }
    }

    // pub(crate) fn is_passable_at(board: &[bool], shape: (usize, usize), pos: [f64; 2]) -> bool {
    //     if pos[0] < 0. || shape.0 <= pos[0] as usize || pos[1] < 0. || shape.1 <= pos[1] as usize {
    //         false
    //     } else {
    //         board[pos[0] as usize + pos[1] as usize * shape.0]
    //     }
    // }

    /// Returns an RGB image and its dimensions
    pub fn occupancy_image(
        &self,
        fog_active: &[bool; 2],
        colored_fog: bool,
    ) -> Option<([usize; 2], Vec<u8>)> {
        const OBSTACLE_COLOR: u8 = 80u8;
        const BACKGROUND_COLOR: u8 = 191u8;

        if self.params.fow {
            let (fa0, fa1) = (fog_active[0], fog_active[1]);

            Some((
                [self.xs, self.ys],
                self.board
                    .iter()
                    .zip(self.fog[0].fow.iter().zip(self.fog[1].fow.iter()))
                    .map(|(p, (f0, f1))| {
                        let c = if *p { BACKGROUND_COLOR } else { OBSTACLE_COLOR };

                        let age_map = |time| {
                            let age = self.global_time.saturating_sub(time);
                            if age == 0 {
                                c
                            } else if age < FOG_MAX_AGE {
                                (c as i32 * 2 / 3) as u8
                            } else {
                                c / 2
                            }
                        };

                        if !fa0 && !fa1 {
                            [c, c, c]
                        } else if colored_fog {
                            if fa0 && fa1 {
                                let (c0, c1) = (age_map(*f0), age_map(*f1));
                                [c1 / 2, c0 / 2, c1.max(c0)]
                            } else if fa0 {
                                let c0 = age_map(*f0);
                                [c / 4, c0 / 2, c0]
                            } else {
                                let c1 = age_map(*f1);
                                [c1 / 2, c / 4, c1]
                            }
                        } else {
                            let age = if fa0 && fa1 {
                                *f0.max(f1)
                            } else if fa0 {
                                *f0
                            } else {
                                *f1
                            };
                            let cc = age_map(age);
                            [cc / 2, cc / 2, cc]
                        }
                    })
                    .flatten()
                    .collect::<Vec<_>>(),
            ))
        } else {
            Some((
                [self.xs, self.ys],
                self.board
                    .iter()
                    .map(|p| [if *p { BACKGROUND_COLOR } else { OBSTACLE_COLOR }; 3])
                    .flatten()
                    .collect::<Vec<_>>(),
            ))
        }
    }

    pub fn labeled_image(&self) -> Option<([usize; 2], Vec<u8>)> {
        let mut rng = Xor128::new(616516);
        let max_label = *self.mesh.labeled_image.iter().max()? + 1;

        const OBSTACLE_COLOR: u8 = 63u8;

        let label_colors = (0..max_label)
            .map(|label| {
                if label == 0 {
                    [OBSTACLE_COLOR, OBSTACLE_COLOR, OBSTACLE_COLOR]
                } else {
                    [
                        (rng.nexti() % 0x80) as u8,
                        (rng.nexti() % 0x80) as u8,
                        (rng.nexti() % 0x80) as u8,
                    ]
                }
            })
            .collect::<Vec<_>>();
        Some((
            [self.xs, self.ys],
            self.mesh
                .labeled_image
                .iter()
                .map(|p| label_colors[*p as usize].into_iter())
                .flatten()
                .collect::<Vec<_>>(),
        ))
    }

    pub fn get_entity(&self, id: usize) -> Option<std::cell::Ref<Entity>> {
        self.entities.iter().find_map(|entity| {
            let entity = entity.borrow();
            if entity.get_id() == id {
                Some(entity)
            } else {
                None
            }
        })
    }
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

pub fn is_passable_at(board: &[bool], shape: (usize, usize), pos: [f64; 2]) -> bool {
    let pos = [pos[0] as isize, pos[1] as isize];
    if pos[0] < 0 || shape.0 as isize <= pos[0] || pos[1] < 0 || shape.1 as isize <= pos[1] {
        false
    } else {
        let pos = [pos[0] as usize, pos[1] as usize];
        board[pos[0] + shape.0 * pos[1]]
    }
}

/// An integer variant of `is_passable_at`.
pub fn is_passable_at_i(board: &[bool], shape: (usize, usize), pos: impl Into<[i32; 2]>) -> bool {
    let pos = pos.into();
    let pos = [pos[0] as isize, pos[1] as isize];
    if pos[0] < 0 || shape.0 as isize <= pos[0] || pos[1] < 0 || shape.1 as isize <= pos[1] {
        false
    } else {
        let pos = [pos[0] as usize, pos[1] as usize];
        board[pos[0] + shape.0 * pos[1]]
    }
}
