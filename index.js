import init, { wasm_main } from "./pkg/swarm_rs.js";

async function run() {
  await init();
  wasm_main();
}

run();
