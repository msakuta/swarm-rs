use std::{collections::BinaryHeap, fmt::Display};

pub(crate) trait DijkstraField {
    fn is_filled(&self, idx: usize) -> bool;
    fn fill(&mut self, idx: usize, cost: i32);
}

impl DijkstraField for &mut [i32] {
    fn is_filled(&self, idx: usize) -> bool {
        self[idx] != std::i32::MAX
    }

    fn fill(&mut self, idx: usize, cost: i32) {
        self[idx] = cost;
    }
}

pub(crate) fn dijkstra_fill(
    board: &[bool],
    shape: (usize, usize),
    start: [i32; 2],
    costmap: &mut impl DijkstraField,
) {
    #[derive(Debug, Clone, Copy)]
    struct OpenState {
        idx: [i32; 2],
        cost: i32,
    }

    impl Display for OpenState {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{:?}", self.idx)
        }
    }

    impl PartialEq for OpenState {
        fn eq(&self, other: &Self) -> bool {
            self.idx == other.idx
        }
    }

    impl Eq for OpenState {}

    impl PartialOrd for OpenState {
        fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
            self.cost.partial_cmp(&other.cost).map(|o| o.reverse())
        }
    }

    impl Ord for OpenState {
        fn cmp(&self, other: &Self) -> std::cmp::Ordering {
            self.cost
                .partial_cmp(&other.cost)
                .unwrap_or(std::cmp::Ordering::Equal)
                .reverse()
        }
    }

    let mut queue = BinaryHeap::new();
    queue.push(OpenState {
        idx: start,
        cost: 0,
    });
    costmap.fill(start[0] as usize + start[1] as usize * shape.0, 0);

    const DIRECTIONS: [[i32; 2]; 4] = [[-1, 0], [0, -1], [1, 0], [0, 1]];

    while let Some(popped) = queue.pop() {
        for dir in DIRECTIONS {
            let next = OpenState {
                idx: [popped.idx[0] + dir[0], popped.idx[1] + dir[1]],
                cost: popped.cost + 1,
            };
            if next.idx[0] < 0
                || shape.0 as i32 <= next.idx[0]
                || next.idx[1] < 0
                || shape.1 as i32 <= next.idx[1]
            {
                continue;
            }
            let idx = next.idx[0] as usize + next.idx[1] as usize * shape.0;
            if costmap.is_filled(idx) {
                continue;
            }
            if board[idx] {
                costmap.fill(
                    next.idx[0] as usize + next.idx[1] as usize * shape.0,
                    next.cost,
                );
                queue.push(next);
            }
        }
    }
}

pub(crate) fn label(board: &[bool], shape: (usize, usize)) -> Vec<i32> {
    let mut labels = vec![0; board.len()];
    let mut label_counter = 1;

    struct Label<'a>(&'a mut [i32], i32, (usize, usize));

    impl<'a> DijkstraField for Label<'a> {
        fn is_filled(&self, idx: usize) -> bool {
            self.0[idx] != 0
        }

        fn fill(&mut self, idx: usize, _: i32) {
            self.0[idx] = self.1;
        }
    }

    for y in 0..shape.1 {
        for x in 0..shape.0 {
            let idx = x + y * shape.0;
            if board[idx] && labels[idx] == 0 {
                dijkstra_fill(
                    board,
                    shape,
                    [x as i32, y as i32],
                    &mut Label(&mut labels[..], label_counter, shape),
                );
                label_counter += 1;
            }
        }
    }

    labels
}
