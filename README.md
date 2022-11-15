# swarm-rs

Swarm game in Rust

Try it now on your browser!
https://msakuta.github.io/swarm-rs/

## Screenshot

![screenshot](https://msakuta.github.io/images/showcase/swarm-rs.png)


## Overview

This is a sister project of [swarm-js](https://github.com/msakuta/swarm-js) but implemented in Rust.

It is a simulation game environment that you can customize the simulated agents behavior via behavior tree.

This project utilizes a behavior tree via [behavior-tree-lite](https://github.com/msakuta/rusty-behavior-tree-lite) crate to define the behavior of the swarm agents.
It is dynamically configurable at runtime.

```
tree main = Sequence {
    Fallback {
        HasTarget (target <- target)
        FindEnemy
    }
    Fallback {
        HasPath (has_path <- has_path)
        FindPath
    }
    Sequence {
        HasPath (has_path <- has_path)
        Fallback {
            FollowPath
            ReactiveSequence {
                Move (direction <- "backward")
                Timeout (time <- "10")
            }
        }
        Shoot
    }
}
```

![screenshot](https://msakuta.github.io/images/showcase/swarm-rs02.png)

Honestly, Druid's text editor widget is not very great, but it works, at least for a simple tree.

I may revisit and implement graphical editor for the behavior tree, but for now
the text editor does its job.
