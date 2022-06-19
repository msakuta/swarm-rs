import init, { wasm_main } from "./swarm_rs.js";

async function run() {
  await init();
  wasm_main();
}

run();
