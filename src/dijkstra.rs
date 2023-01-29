use std::{collections::BinaryHeap, fmt::Display};

pub(crate) fn dijkstra_fill(
    board: &[bool],
    shape: (usize, usize),
    start: [i32; 2],
    costmap: &mut [i32],
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

    const DIRECTIONS: [[i32; 2]; 4] = [[-1, 0], [0, -1], [1, 0], [0, 1]];

    while let Some(popped) = queue.pop() {
        costmap[popped.idx[0] as usize + popped.idx[1] as usize * shape.0] = popped.cost;
        for dir in DIRECTIONS {
            let next = OpenState {
                idx: [popped.idx[0] + dir[0], popped.idx[1] + dir[1]],
                cost: popped.cost + 1,
            };
            if next.idx[0] < 0
                || shape.0 <= next.idx[0] as usize
                || next.idx[1] < 0
                || shape.1 <= next.idx[1] as usize
            {
                continue;
            }
            let idx = next.idx[0] as usize + next.idx[1] as usize * shape.0;
            if costmap[idx] != std::i32::MAX {
                continue;
            }
            if board[idx] {
                queue.push(next);
            }
        }
    }
}
