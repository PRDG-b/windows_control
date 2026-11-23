use rerun::{
    EncodedImage, GraphEdges, GraphNodes,
    Points2D, RecordingStream, TextLog,
};

use crate::exploration::{EdgeOutcome, Graph};

pub type LogResult = Result<(), Box<dyn std::error::Error>>;

pub fn log_step(
    rec: &RecordingStream,
    step: i64,
    jpeg: &[u8],
    graph: &Graph,
    current: &str,
    event: &str,
    click: Option<[f32; 2]>,
) -> LogResult {
    // 시간축 설정
    rec.set_time_sequence("step", step);

    // 캡처 화면 로깅
    rec.log(
        "screen",
        &EncodedImage::from_file_contents(jpeg.to_vec())
            .with_media_type("image/jpeg"),
    )?;

    // 클릭 위치
    rec.log(
        "screen/click",
        &Points2D::new(click.into_iter())
            .with_radii([6.0])
            .with_labels(["click"]),
    )?;

    // 누적 노드 추출 및 현재 상태 노드 하이라이트
    // 현재 상태 표시
    let node_ids: Vec<&str> = graph.nodes.iter().map(|n| n.state.as_str()).collect();
    let colors = node_ids.iter().map(|id| {
        if * id == current {
            [255_u8, 80, 80]
        }
        else {
            [150_u8, 150, 150]
        }
    });

    rec.log(
        "graph",
        &GraphNodes::new(node_ids.iter().copied())
            .with_labels(node_ids.iter().copied())
            .with_colors(colors),
    )?;

    // 유효 간선 추출
    let mut edge_tuples = Vec::new();
    for edge in &graph.edges {
        if let Some(to_idx) = edge.to {
            if edge.outcome == EdgeOutcome::Transition {
                let from_state = graph.nodes[edge.from].state.as_str();
                let to_state = graph.nodes[to_idx].state.as_str();
                edge_tuples.push((from_state, to_state));
            }
        }
    }

    rec.log(
        "graph",
        &GraphEdges::new(edge_tuples).with_directed_edges(),
    )?;

    // 이벤트 텍스트
    rec.log(
        "events",
        &TextLog::new(format!("state={current} event={event}")),
    )?;

    Ok(())
}