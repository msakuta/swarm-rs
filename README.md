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

## How to run native application on PC

* Install [Rust](https://www.rust-lang.org/learn/get-started)

We have 2 versions of native application, using different GUI frameworks.

### eframe

* Run `cargo r -p swarm-rs-eframe`

### druid

* Run `cargo r -p swarm-rs-druid`

Note that Druid is being discontinued so we will drop support some time in the future.


## How to build Wasm version

You can build the application to WebAssembly and run on the browser.

* Install [Rust](https://www.rust-lang.org/learn/get-started)
* Install [wasm-pack](https://rustwasm.github.io/wasm-pack/)

We have 2 versions of wasm application, using different GUI frameworks.


### eframe

* Install [trunk](https://github.com/thedodd/trunk) by `cargo install trunk`
* Run `cd eframe && trunk serve` for development server, or
* Run `cd eframe && trunk build --release` for release build in `eframe/dist`

### druid

* Run `cd druid && wasm-pack build --release`
* Copy `druid/index.html` and `druid/index.js` to `druid/pkg`


## How to edit the behavior tree

There are tabs to switch the main panel to the editors on right top corner of the window.
Each tab represents the type of the entities that use the behavior.

You can edit the tree in-place with the integrated text editor, or edit it in a text editor and reload from file.
If you want to use VSCode to edit, check out the [syntax highlighting extension](https://github.com/msakuta/rusty-behavior-tree-lite/tree/master/vscode-ext).

![editor-screenshot](screenshots/behavior-tree-editors.png)

Currently, loading from file is not supported in Wasm version.
