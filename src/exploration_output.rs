//! Text artifacts contain controlled state/action IDs and coordinates, not OCR.
use crate::exploration::ExplorationReport;
use std::{fmt::Write, path::Path};

pub fn write_report(report: &ExplorationReport, dir: &Path, live: bool) -> Result<(), String> {
    let mode = if live { "hdmi_hid" } else { "simulation" };
    let summary = format!(
        "{{\"mode\":\"{mode}\",\"stop\":\"{}\",\"nodes\":{},\"edges\":{},\"probes\":{},\"input_attempts\":{},\"observed_frames\":{},\"restored\":{},\"pending\":{},\"elapsed_ms\":{}}}",
        report.stop.code(),
        report.graph.nodes.len(),
        report.graph.edges.len(),
        report.probes,
        report.input_attempts,
        report.observed_frames,
        report.restored,
        report.pending.len(),
        report.elapsed.as_millis()
    );
    let mut dot = String::from("digraph exploration {\n  rankdir=LR;\n");
    let mut mermaid = String::from("flowchart LR\n");
    let mut nodes = String::from("node_id\tstate_id\n");
    for (id, node) in report.graph.nodes.iter().enumerate() {
        writeln!(dot, "  n{id} [label=\"{}\"];", node.state).unwrap();
        writeln!(mermaid, "  n{id}[\"{}\"]", node.state).unwrap();
        writeln!(nodes, "{id}\t{}", node.state).unwrap();
    }
    let mut edges = String::from("from\tto\taction\tx\ty\tbefore_frame\tafter_frame\toutcome\n");
    for (id, edge) in report.graph.edges.iter().enumerate() {
        let target = edge.to.map(|n| format!("n{n}")).unwrap_or(format!("u{id}"));
        if edge.to.is_none() {
            writeln!(dot, "  {target} [label=\"unconfirmed\",shape=box];").unwrap();
            writeln!(mermaid, "  {target}[\"unconfirmed\"]").unwrap();
        }
        writeln!(
            dot,
            "  n{} -> {target} [label=\"{} / {}\"];",
            edge.from,
            edge.action,
            edge.outcome.code()
        )
        .unwrap();
        writeln!(
            mermaid,
            "  n{} -->|\"{} / {}\"| {target}",
            edge.from,
            edge.action,
            edge.outcome.code()
        )
        .unwrap();
        writeln!(
            edges,
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            edge.from,
            edge.to.map(|n| n.to_string()).unwrap_or_default(),
            edge.action,
            edge.point.0,
            edge.point.1,
            edge.before_frame,
            edge.after_frame.map(|n| n.to_string()).unwrap_or_default(),
            edge.outcome.code()
        )
        .unwrap();
    }
    dot.push_str("}\n");
    let mut attempts = String::from("sequence\tkind\taction\tframe\tx\ty\tacknowledged\n");
    for (i, a) in report.attempts.iter().enumerate() {
        let (x, y) = a
            .point
            .map(|(x, y)| (x.to_string(), y.to_string()))
            .unwrap_or_default();
        writeln!(
            attempts,
            "{}\t{}\t{}\t{}\t{x}\t{y}\t{}",
            i + 1,
            a.kind,
            a.action,
            a.frame.map(|f| f.to_string()).unwrap_or_default(),
            a.acknowledged
        )
        .unwrap();
    }
    let mut pending = String::from("node\taction\n");
    for (node, action) in &report.pending {
        writeln!(pending, "{node}\t{action}").unwrap();
    }
    let mut skipped = String::from("node\taction\treason\n");
    for s in &report.skipped {
        writeln!(skipped, "{}\t{}\t{}", s.node, s.candidate, s.reason).unwrap();
    }
    for (name, data) in [
        ("report.json", format!("{summary}\n")),
        ("graph.dot", dot),
        ("graph.mmd", mermaid),
        ("nodes.tsv", nodes),
        ("edges.tsv", edges),
        ("attempts.tsv", attempts),
        ("pending.tsv", pending),
        ("skipped.tsv", skipped),
    ] {
        std::fs::write(dir.join(name), data)
            .map_err(|e| format!("Cannot save exploration artifact: {e}"))?;
    }
    println!("{summary}");
    Ok(())
}
