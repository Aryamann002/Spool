//! Reproducible core timings, not GPUI frame-time benchmarks.
//! Run: cargo test --locked core_workload_baseline -- --ignored --nocapture --test-threads=1
use super::*;
use crate::operations::{SemanticHistory, SemanticOperation};
use std::{hint::black_box, time::Instant};

fn measure(mut operation: impl FnMut(), iterations: usize) -> f64 {
    for _ in 0..10 {
        operation();
    }
    let start = Instant::now();
    for _ in 0..iterations {
        operation();
    }
    start.elapsed().as_secs_f64() * 1_000_000.0 / iterations as f64
}

#[test]
#[ignore = "timing harness; run explicitly on an idle machine"]
fn core_workload_baseline() {
    println!("microseconds/operation; debug profile; 1000 iterations; 10 warmups");
    for count in [100, 1_000, 10_000] {
        // Build outside timed regions, without exercising quadratic insertion.
        let mut document = Document::default();
        document.objects = (0..count)
            .map(|index| DesignObject {
                id: ObjectId(index as u64 + 5),
                spool_id: node_id(format!("spool-bench-{index:016x}")),
                name: format!("Text {index}"),
                position: point((index % 100) as f32 * 40.0, (index / 100) as f32 * 40.0),
                size: size(30.0, 30.0),
                object_type: ObjectType::Text,
                text_content: Some("Representative text content".repeat(4)),
                fill: default_style(ObjectType::Text).fill,
                stroke: None,
            })
            .collect();
        let clone = measure(
            || {
                black_box(black_box(&document).clone());
            },
            1000,
        );
        let borrow = measure(
            || {
                black_box(black_box(&document).objects());
            },
            1000,
        );
        let hit = measure(
            || {
                black_box(document.hit_test(black_box(point(-1.0, -1.0))));
            },
            1000,
        );
        let marquee = measure(
            || {
                black_box(document.objects_in(black_box(WorldRect::from_points(
                    point(0.0, 0.0),
                    point(400.0, 400.0),
                ))));
            },
            1000,
        );
        let snapshots: Vec<_> = document
            .objects
            .iter()
            .rev()
            .take(10)
            .map(|object| ObjectSnapshot {
                id: object.id,
                geometry: object.geometry(),
            })
            .collect();
        let movement = measure(
            || {
                apply_move(&mut document, &snapshots, black_box(point(10.0, 10.0)));
            },
            1000,
        );
        let history = measure(
            || {
                let mut history = SemanticHistory::default();
                history.record(SemanticOperation::Runtime(geometry_command(
                    &document, &snapshots,
                )));
                black_box(history);
            },
            1000,
        );
        println!("N={count}: snapshot_clone={clone:.3}, render_borrow={borrow:.3}, hit_miss={hit:.3}, marquee={marquee:.3}, move_10={movement:.3}, history_10={history:.3}");
    }
}
