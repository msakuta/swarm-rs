use cgmath::{InnerSpace, Vector2};

use druid::Data;
use std::{cell::RefCell, rc::Rc};

use crate::{
    agent::{Agent, AgentState, Bullet},
    collision::CollisionShape,
    entity::{Entity, GameEvent},
    measure_time,
    mesh::{create_mesh, Mesh, MeshResult},
    perlin_noise::{gen_terms, perlin_noise_pixel, Xor128},
    spawner::Spawner,
    temp_ents::TempEnt,
    triangle_utils::{check_shape_in_mesh, find_triangle_at},
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
    pub(crate) mesh: Rc<Mesh>,
    pub(crate) entities: Rc<RefCell<Vec<RefCell<Entity>>>>,
    pub(crate) bullets: Rc<Vec<Bullet>>,
    pub(crate) paused: bool,
    pub(crate) interval: f64,
    pub(crate) rng: Rc<Xor128>,
    pub(crate) id_gen: usize,
    pub(crate) temp_ents: Rc<RefCell<Vec<TempEnt>>>,
    pub(crate) triangle_profiler: Rc<RefCell<Profiler>>,
    pub(crate) pixel_profiler: Rc<RefCell<Profiler>>,
    pub(crate) source: Rc<String>,
}

impl Game {
    pub(crate) fn new() -> Self {
        let seed = 123513;
        let simplify = 1.;

        let xs = 128;
        let ys = 128;

        let MeshResult { board, mesh } = Self::create_perlin_board((xs, ys), seed, simplify);

        let id_gen = 0;

        Self {
            xs,
            ys,
            simplify,
            board: Rc::new(board),
            mesh: Rc::new(mesh),
            entities: Rc::new(RefCell::new(vec![])),
            bullets: Rc::new(vec![]),
            paused: false,
            interval: 32.,
            rng: Rc::new(Xor128::new(9318245)),
            id_gen,
            temp_ents: Rc::new(RefCell::new(vec![])),
            triangle_profiler: Rc::new(RefCell::new(Profiler::new())),
            pixel_profiler: Rc::new(RefCell::new(Profiler::new())),
            source: Rc::new(String::new()),
        }
    }

    pub fn create_perlin_board(
        shape: (usize, usize),
        seed: u32,
        simplify_epsilon: f64,
    ) -> MeshResult {
        let bits = 6;
        let mut xor128 = Xor128::new(seed);
        let terms = gen_terms(&mut xor128, bits);

        create_mesh(shape, simplify_epsilon, |xi, yi| {
            let dx = (xi as isize - shape.0 as isize / 2) as f64;
            let dy = (yi as isize - shape.1 as isize / 2) as f64;
            let noise_val = perlin_noise_pixel(xi as f64, yi as f64, bits, &terms, 0.5);
            0. + (noise_val - 0.5 * (dx * dx + dy * dy).sqrt() / shape.0 as f64) > -0.125
        })
    }

    pub fn create_rect_board(
        shape: (usize, usize),
        _seed: u32,
        simplify_epsilon: f64,
    ) -> MeshResult {
        let (xs, ys) = (shape.0 as isize, shape.1 as isize);
        create_mesh(shape, simplify_epsilon, |xi, yi| {
            let dx = xi as isize - xs / 2;
            let dy = yi as isize - ys / 2;
            dx.abs() < xs / 4 && dy.abs() < ys / 4
        })
    }

    pub fn create_crank_board(
        shape: (usize, usize),
        _seed: u32,
        simplify_epsilon: f64,
    ) -> MeshResult {
        let (xs, ys) = (shape.0 as isize, shape.1 as isize);
        create_mesh(shape, simplify_epsilon, |xi, yi| {
            let dx = xi as isize - xs / 2;
            let dy = yi as isize - ys / 2;
            dx.abs() < xs * 3 / 8
                && dy.abs() < ys / 8
                && !(-xs * 3 / 16 < dx && dx < -xs * 2 / 16 && -ys / 16 < dy)
                && !(xs * 2 / 16 < dx && dx < xs * 3 / 16 && dy < ys / 16)
        })
    }

    pub(crate) fn new_board(&mut self, shape: (usize, usize), seed: u32, simplify: f64) {
        self.xs = shape.0;
        self.ys = shape.0;
        let MeshResult { board, mesh } = Self::create_crank_board(shape, seed, simplify);

        self.board = Rc::new(board);
        self.mesh = Rc::new(mesh);
        self.entities = Rc::new(RefCell::new(vec![]));
        self.bullets = Rc::new(vec![]);
    }

    pub(crate) fn try_new_agent(
        &mut self,
        pos: [f64; 2],
        team: usize,
        entities: &[RefCell<Entity>],
        static_: bool,
    ) -> Option<Entity> {
        const STATIC_SOURCE_FILE: &str = include_str!("../test_obstacle.txt");
        let rng = Rc::make_mut(&mut self.rng);
        let id_gen = &mut self.id_gen;
        let triangle_labels = &self.mesh.triangle_labels;
        let largest_label = self.mesh.largest_label;
        for _ in 0..10 {
            let state_candidate = AgentState {
                x: pos[0], // + rng.next() * 10. - 5.,
                y: pos[1], // + rng.next() * 10. - 5.,
                heading: rng.next() * std::f64::consts::PI * 2.,
            };

            if Agent::collision_check(None, state_candidate, entities, false) {
                continue;
            }

            if let Some(tri) = find_triangle_at(
                &self.mesh,
                state_candidate.into(),
                &mut *self.triangle_profiler.borrow_mut(),
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
        check_shape_in_mesh(
            &self.mesh,
            &state,
            &mut *self.triangle_profiler.borrow_mut(),
        )
    }

    fn try_new_spawner(&mut self, team: usize) -> Option<Entity> {
        for _ in 0..10 {
            let rng = Rc::make_mut(&mut self.rng);
            let pos_candidate = [rng.next() * self.xs as f64, rng.next() * self.ys as f64];
            if let Some(tri) = find_triangle_at(
                &self.mesh,
                pos_candidate,
                &mut *self.triangle_profiler.borrow_mut(),
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
                    if let Some(agent) = self.try_new_agent(pos, team, &entities, false) {
                        entities.push(RefCell::new(agent));
                    }
                }
            }
        }

        *self.entities.borrow_mut() = entities;
        self.bullets = Rc::new(bullets);

        {
            let agents = self.entities.as_ref().borrow();
            let mut temp_ents = std::mem::take(&mut *self.temp_ents.borrow_mut());
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
                    .collect(),
            );

            *self.temp_ents.borrow_mut() = temp_ents
                .into_iter()
                .filter_map(|mut ent| if ent.update() { Some(ent) } else { None })
                .collect();
        }

        let mut entities: Vec<_> = std::mem::take(&mut *self.entities.borrow_mut())
            .into_iter()
            .filter(|agent| agent.borrow().get_active())
            .collect();

        if entities.is_empty() {
            println!("Adding agents");
            let pos = [self.xs as f64 * 2. / 8., self.ys as f64 * 9. / 16.];
            if let Some(agent) = self.try_new_agent(pos, 0, &entities, false) {
                entities.push(RefCell::new(agent));
            }
            let pos = [self.xs as f64 / 2., self.ys as f64 / 2.];
            if let Some(agent) = self.try_new_agent(pos, 0, &entities, true) {
                entities.push(RefCell::new(agent));
            }
        }

        // for team in 0..2 {
        //     let rng = Rc::make_mut(&mut self.rng);
        //     if entities
        //         .iter()
        //         .filter(|agent| !agent.borrow().is_agent() && agent.borrow().get_team() == team)
        //         .count()
        //         < 1
        //         && rng.next() < 0.1
        //     {
        //         if let Some(spawner) = self.try_new_spawner(team) {
        //             entities.push(RefCell::new(spawner));
        //         }
        //     }
        // }
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
