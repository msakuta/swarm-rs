use cgmath::{MetricSpace, Vector2};

use crate::agent::interpolation;

use super::{CellState, Rect};
use std::{
    collections::HashMap,
    sync::atomic::{AtomicUsize, Ordering},
};

/// A bitmap to update quad tree using only changed parts of the map.
///
/// It has array index indirection to reduce the map size, since CellState tends to be
/// much larger than u32.
#[derive(Debug)]
pub struct CacheMap {
    /// An internal map having the size (2 ^ toplevel) ^ 2, indicating index into [`cache_buf`]
    map: Vec<u32>,
    /// An array of actual values in [`cache_map`], extracted to reduce the pixel size.
    buf: Vec<CellState>,
    topbit: usize,
    size: usize,
    /// A history of recently updated cells for visualization
    pub fresh_cells: HashMap<[i32; 2], usize>,
    prev_map: Option<Vec<u32>>,
}

pub const FRESH_TICKS: usize = 8;

static QUERY_CALLS: AtomicUsize = AtomicUsize::new(0);
static UNPASSABLES: AtomicUsize = AtomicUsize::new(0);

impl CacheMap {
    pub(super) fn new() -> Self {
        Self {
            map: vec![],
            buf: vec![],
            topbit: 0,
            size: 0,
            fresh_cells: HashMap::new(),
            prev_map: None,
        }
    }

    pub fn get(&self, pos: [i32; 2]) -> CellState {
        self.buf[self.map[pos[0] as usize + pos[1] as usize * self.size] as usize]
    }

    pub(super) fn cache(
        &mut self,
        topbit: usize,
        shape: (usize, usize),
        f: &impl Fn(Rect) -> CellState,
    ) {
        self.topbit = topbit;
        self.size = 1 << topbit;
        self.map = vec![0; self.size * self.size];
        for y in 0..shape.1 {
            for x in 0..shape.0 {
                let pix = f([x as i32, y as i32, x as i32 + 1, y as i32 + 1]);
                if let Some((idx, _)) = self.buf.iter().enumerate().find(|(_, b)| **b == pix) {
                    self.map[x + y * self.size] = idx as u32;
                } else {
                    self.map[x + y * self.size] = self.buf.len() as u32;
                    self.buf.push(pix);
                }
            }
        }
        dbg_println!(
            "cache_map size: {}, cache_buf: {:?}",
            std::mem::size_of::<u32>() * self.map.len(),
            self.buf
        );
    }

    pub(super) fn start_update(&mut self) {
        if self
            .prev_map
            .as_ref()
            .map(|prev_map| prev_map.len() != self.map.len())
            .unwrap_or(true)
        {
            self.prev_map = Some(self.map.clone());
        }
    }

    pub(super) fn update(&mut self, pos: [i32; 2], pix: CellState) -> Result<bool, String> {
        if pos[0] < 0 || self.size as i32 <= pos[0] || pos[1] < 0 || self.size as i32 <= pos[1] {
            return Err("Out of bounds!".to_string());
        }

        let existing = &mut self.map[pos[0] as usize + pos[1] as usize * self.size];

        if self.buf[*existing as usize] != pix {
            // dbg_println!("Updating cache_map {pos:?}: {:?} -> {:?}", *existing, pix);

            if let Some((idx, _)) = self.buf.iter().enumerate().find(|(_, buf)| **buf == pix) {
                *existing = idx as u32;
            } else {
                let idx = self.buf.len();
                self.buf.push(pix);
                *existing = idx as u32;
            }

            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub(super) fn finish_update(&mut self) {
        let Some(prev_map) = &mut self.prev_map else {
            return;
        };

        for i in self
            .map
            .iter()
            .zip(prev_map.iter_mut())
            .enumerate()
            .filter_map(|(i, (cur, prev))| {
                if *cur != *prev {
                    *prev = *cur;
                    Some(i)
                } else {
                    None
                }
            })
        {
            self.fresh_cells.insert(
                [(i % self.size) as i32, (i / self.size) as i32],
                FRESH_TICKS,
            );
        }

        let mut to_delete = vec![];
        for fresh_cell in &mut self.fresh_cells {
            if 1 < *fresh_cell.1 {
                *fresh_cell.1 -= 1;
            } else {
                to_delete.push(*fresh_cell.0);
            }
        }

        for to_delete in to_delete {
            self.fresh_cells.remove(&to_delete);
        }
    }

    pub(super) fn query(&self, rect: Rect) -> CellState {
        let mut has_passable = false;
        let mut has_unpassable = None;
        for x in rect[0]..rect[2] {
            for y in rect[1]..rect[3] {
                QUERY_CALLS.fetch_add(1, Ordering::Relaxed);
                let mut has_unpassable_local = None;
                let pix = &self.buf[self.map[x as usize + y as usize * self.size] as usize];
                if !matches!(pix, CellState::Free) {
                    UNPASSABLES.fetch_add(1, Ordering::Relaxed);
                    has_unpassable_local = Some(*pix);
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
    }

    pub fn is_position_visible(
        &self,
        collide: impl Fn(CellState) -> bool,
        source: [f64; 2],
        target: [f64; 2],
    ) -> bool {
        const INTERPOLATE_INTERVAL: f64 = 1.;
        let distance = Vector2::from(source).distance(Vector2::from(target));
        if distance < INTERPOLATE_INTERVAL {
            return false;
        }
        !interpolation::interpolate(source, target, INTERPOLATE_INTERVAL, |point| {
            if point[0] < 0.
                || self.size <= point[0] as usize
                || point[1] < 0.
                || self.size <= point[1] as usize
            {
                true
            } else {
                let cell = self.get([point[0] as i32, point[1] as i32]);
                collide(cell)
            }
        })
    }
}
