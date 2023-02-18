import init, { wasm_main } from "./swarm_rs_druid.js";

async function run() {
  await init();
  wasm_main();
}

run();
