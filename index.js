import init, { wasm_main } from "./pkg/mesh_transform_rs.js";

async function run() {
  await init();
  wasm_main();
}

run();
