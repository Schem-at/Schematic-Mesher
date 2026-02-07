//! Performance benchmark for the meshing pipeline.
//!
//! Usage:
//!   cargo run --release --example benchmark                  # run_label=baseline
//!   cargo run --release --example benchmark -- optimized_v1  # run_label=optimized_v1
//!
//! Requires `pack.zip` in the project root.
//! Results append to `artifacts/benchmark.csv`.

use schematic_mesher::{
    load_resource_pack, BlockPosition, BoundingBox, InputBlock, Mesher, MesherConfig,
};
use std::time::Instant;

/// A scene to benchmark.
struct Scene {
    name: &'static str,
    blocks: Vec<(BlockPosition, InputBlock)>,
    bounds: BoundingBox,
}

fn make_stone() -> InputBlock {
    InputBlock::new("minecraft:stone")
}

fn make_granite() -> InputBlock {
    InputBlock::new("minecraft:granite")
}

fn make_diorite() -> InputBlock {
    InputBlock::new("minecraft:diorite")
}

fn make_andesite() -> InputBlock {
    InputBlock::new("minecraft:andesite")
}

/// 16 common block types for random_blocks scene.
fn random_block_types() -> Vec<InputBlock> {
    vec![
        InputBlock::new("minecraft:stone"),
        InputBlock::new("minecraft:granite"),
        InputBlock::new("minecraft:diorite"),
        InputBlock::new("minecraft:andesite"),
        InputBlock::new("minecraft:dirt"),
        InputBlock::new("minecraft:cobblestone"),
        InputBlock::new("minecraft:oak_planks"),
        InputBlock::new("minecraft:spruce_planks"),
        InputBlock::new("minecraft:birch_planks"),
        InputBlock::new("minecraft:sandstone"),
        InputBlock::new("minecraft:bricks"),
        InputBlock::new("minecraft:mossy_cobblestone"),
        InputBlock::new("minecraft:obsidian"),
        InputBlock::new("minecraft:netherrack"),
        InputBlock::new("minecraft:end_stone"),
        InputBlock::new("minecraft:prismarine"),
    ]
}

/// N×N×N solid cube of stone.
fn solid_cube(n: i32) -> Scene {
    let mut blocks = Vec::with_capacity((n * n * n) as usize);
    for x in 0..n {
        for y in 0..n {
            for z in 0..n {
                blocks.push((BlockPosition::new(x, y, z), make_stone()));
            }
        }
    }
    Scene {
        name: "solid_cube",
        blocks,
        bounds: BoundingBox::new([0.0; 3], [n as f32; 3]),
    }
}

/// N×N×N hollow shell (1-block thick).
fn hollow_cube(n: i32) -> Scene {
    let mut blocks = Vec::new();
    for x in 0..n {
        for y in 0..n {
            for z in 0..n {
                if x == 0 || x == n - 1 || y == 0 || y == n - 1 || z == 0 || z == n - 1 {
                    blocks.push((BlockPosition::new(x, y, z), make_stone()));
                }
            }
        }
    }
    Scene {
        name: "hollow_cube",
        blocks,
        bounds: BoundingBox::new([0.0; 3], [n as f32; 3]),
    }
}

/// Alternating stone/air checkerboard.
fn checkerboard(n: i32) -> Scene {
    let mut blocks = Vec::new();
    for x in 0..n {
        for y in 0..n {
            for z in 0..n {
                if (x + y + z) % 2 == 0 {
                    blocks.push((BlockPosition::new(x, y, z), make_stone()));
                }
            }
        }
    }
    Scene {
        name: "checkerboard",
        blocks,
        bounds: BoundingBox::new([0.0; 3], [n as f32; 3]),
    }
}

/// N×N×N with 4 block types in a repeating pattern.
fn mixed_blocks(n: i32) -> Scene {
    let mut blocks = Vec::with_capacity((n * n * n) as usize);
    let types = [make_stone(), make_granite(), make_diorite(), make_andesite()];
    for x in 0..n {
        for y in 0..n {
            for z in 0..n {
                let idx = ((x + y * 2 + z * 3) % 4) as usize;
                blocks.push((BlockPosition::new(x, y, z), types[idx].clone()));
            }
        }
    }
    Scene {
        name: "mixed_blocks",
        blocks,
        bounds: BoundingBox::new([0.0; 3], [n as f32; 3]),
    }
}

/// N×N×1 flat stone floor.
fn flat_plane(n: i32) -> Scene {
    let mut blocks = Vec::with_capacity((n * n) as usize);
    for x in 0..n {
        for z in 0..n {
            blocks.push((BlockPosition::new(x, 0, z), make_stone()));
        }
    }
    Scene {
        name: "flat_plane",
        blocks,
        bounds: BoundingBox::new([0.0; 3], [n as f32, 1.0, n as f32]),
    }
}

/// N×N×N with 16 different block types chosen pseudo-randomly.
/// Worst case for model resolution cache (many unique types).
fn random_blocks(n: i32) -> Scene {
    let types = random_block_types();
    let num_types = types.len();
    let mut blocks = Vec::with_capacity((n * n * n) as usize);
    for x in 0..n {
        for y in 0..n {
            for z in 0..n {
                // Pseudo-random selection using a simple hash
                let idx = ((x.wrapping_mul(73856093)) ^ (y.wrapping_mul(19349669)) ^ (z.wrapping_mul(83492791)))
                    .unsigned_abs() as usize
                    % num_types;
                blocks.push((BlockPosition::new(x, y, z), types[idx].clone()));
            }
        }
    }
    Scene {
        name: "random_blocks",
        blocks,
        bounds: BoundingBox::new([0.0; 3], [n as f32; 3]),
    }
}

struct BenchResult {
    scene: String,
    size: i32,
    config: String,
    total_blocks: usize,
    mesh_ms: f64,
    glb_export_ms: f64,
    usdz_export_ms: f64,
    triangles: usize,
    vertices: usize,
    greedy_materials: usize,
}

fn run_bench(
    pack: &schematic_mesher::ResourcePack,
    scene: &Scene,
    size: i32,
    config_name: &str,
    config: MesherConfig,
) -> BenchResult {
    let mesher = Mesher::with_config(pack.clone(), config);

    let total_blocks = scene.blocks.len();

    // Time meshing
    let start = Instant::now();
    let output = mesher
        .mesh_blocks(
            scene.blocks.iter().map(|(pos, block)| (*pos, block)),
            scene.bounds,
        )
        .expect("Meshing failed");
    let mesh_ms = start.elapsed().as_secs_f64() * 1000.0;

    let triangles = output.total_triangles();
    let vertices = output.total_vertices();
    let greedy_materials = output.greedy_materials.len();

    // Time GLB export
    let start = Instant::now();
    let _glb = schematic_mesher::export_glb(&output).ok();
    let glb_export_ms = start.elapsed().as_secs_f64() * 1000.0;

    // Time USDZ export
    let start = Instant::now();
    let _usdz = schematic_mesher::export_usdz(&output).ok();
    let usdz_export_ms = start.elapsed().as_secs_f64() * 1000.0;

    BenchResult {
        scene: scene.name.to_string(),
        size,
        config: config_name.to_string(),
        total_blocks,
        mesh_ms,
        glb_export_ms,
        usdz_export_ms,
        triangles,
        vertices,
        greedy_materials,
    }
}

fn main() {
    let run_label = std::env::args().nth(1).unwrap_or_else(|| "baseline".to_string());

    eprintln!("Loading resource pack...");
    let pack = load_resource_pack("pack.zip").expect("Failed to load pack.zip from project root");

    let sizes: &[i32] = &[4, 8, 16, 32, 64];

    // Scene generators: (name_fn, extra_sizes, skip_baseline_at)
    // skip_baseline_at: if Some(n), skip baseline config at sizes >= n (too slow)
    type SceneFn = fn(i32) -> Scene;
    struct SceneEntry {
        name: &'static str,
        func: SceneFn,
        base_sizes: &'static [i32],
        extra_sizes: &'static [i32],
        skip_baseline_at: Option<i32>,
    }
    let scenes: Vec<SceneEntry> = vec![
        SceneEntry {
            name: "solid_cube",
            func: solid_cube as SceneFn,
            base_sizes: sizes,
            extra_sizes: &[128, 256],
            skip_baseline_at: Some(256), // skip baseline for 256 only
        },
        SceneEntry {
            name: "hollow_cube",
            func: hollow_cube,
            base_sizes: sizes,
            extra_sizes: &[128],
            skip_baseline_at: None,
        },
        SceneEntry {
            name: "checkerboard",
            func: checkerboard,
            base_sizes: sizes,
            extra_sizes: &[128],
            skip_baseline_at: Some(128), // skip baseline for 128 (too slow)
        },
        SceneEntry {
            name: "mixed_blocks",
            func: mixed_blocks,
            base_sizes: sizes,
            extra_sizes: &[128],
            skip_baseline_at: None,
        },
        SceneEntry {
            name: "flat_plane",
            func: flat_plane,
            base_sizes: sizes,
            extra_sizes: &[128],
            skip_baseline_at: None,
        },
        SceneEntry {
            name: "random_blocks",
            func: random_blocks,
            base_sizes: sizes,
            extra_sizes: &[128],
            skip_baseline_at: None,
        },
    ];

    let configs: Vec<(&str, MesherConfig)> = vec![
        (
            "baseline",
            MesherConfig {
                cull_hidden_faces: true,
                cull_occluded_blocks: false,
                greedy_meshing: false,
                ambient_occlusion: true,
                ao_intensity: 0.4,
                atlas_max_size: 4096,
                atlas_padding: 1,
                include_air: false,
                tint_provider: schematic_mesher::TintProvider::new(),
            },
        ),
        (
            "optimized",
            MesherConfig {
                cull_hidden_faces: true,
                cull_occluded_blocks: true,
                greedy_meshing: true,
                ambient_occlusion: true,
                ao_intensity: 0.4,
                atlas_max_size: 4096,
                atlas_padding: 1,
                include_air: false,
                tint_provider: schematic_mesher::TintProvider::new(),
            },
        ),
    ];

    let csv_path = "artifacts/benchmark.csv";
    let write_header = !std::path::Path::new(csv_path).exists();

    let mut csv_file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(csv_path)
        .expect("Failed to open benchmark CSV");

    use std::io::Write;
    if write_header {
        writeln!(
            csv_file,
            "run_label,scene,size,config,total_blocks,mesh_ms,glb_export_ms,usdz_export_ms,triangles,vertices,greedy_materials"
        )
        .unwrap();
    }

    let mut results = Vec::new();

    for entry in &scenes {
        let all_sizes: Vec<i32> = entry
            .base_sizes
            .iter()
            .chain(entry.extra_sizes.iter())
            .copied()
            .collect();

        for &size in &all_sizes {
            let scene = (entry.func)(size);

            for (config_name, config) in &configs {
                // Skip baseline for large sizes where specified
                if *config_name == "baseline" {
                    if let Some(skip_at) = entry.skip_baseline_at {
                        if size >= skip_at {
                            eprintln!(
                                "  {:<15} size={:<4} config={:<12} ... SKIPPED (too slow)",
                                entry.name, size, config_name
                            );
                            continue;
                        }
                    }
                }

                eprint!(
                    "  {:<15} size={:<4} config={:<12} ... ",
                    entry.name, size, config_name
                );

                let result = run_bench(&pack, &scene, size, config_name, config.clone());

                eprintln!(
                    "mesh={:>8.1}ms  glb={:>8.1}ms  usdz={:>8.1}ms  tris={:<8} verts={:<8}",
                    result.mesh_ms,
                    result.glb_export_ms,
                    result.usdz_export_ms,
                    result.triangles,
                    result.vertices,
                );

                writeln!(
                    csv_file,
                    "{},{},{},{},{},{:.3},{:.3},{:.3},{},{},{}",
                    run_label,
                    result.scene,
                    result.size,
                    result.config,
                    result.total_blocks,
                    result.mesh_ms,
                    result.glb_export_ms,
                    result.usdz_export_ms,
                    result.triangles,
                    result.vertices,
                    result.greedy_materials,
                )
                .unwrap();

                results.push(result);
            }
        }
    }

    eprintln!("\nResults appended to {}", csv_path);
    eprintln!("Run label: {}", run_label);
    eprintln!("Total benchmarks: {}", results.len());
}
