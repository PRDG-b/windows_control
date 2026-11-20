use rerun::{
    EncodedImage, GraphEdges, GraphNodes,
    Points2D, RecordingStream, TextLog,
};

type LogResult = Result<(), Box<dyn std::error::Error>>;

fn log_step(
    rec: &RecordingStream,
    step: i64,
    jpeg: &[u8],
    nodes: &[&str],
    edges: &[(&str, &str)],
    current: &str,
    event: &str,
    click: Option<[f32; 2]>,
) -> LogResult {
    // 같은 스레드에서 기록되는 데이터에 적용
    rec.set_time_sequence("step", step);

    // JPEG 디코딩 안하고 전송
    rec.log(
        "screen",
        &EncodedImage::from_file_contents(jpeg.to_vec())
            .with_media_type("image/jpeg"),
    )?;

    // None이면 목록 기록하여 이전 클릭 제거
    rec.log(
        "screen/click",
        &Points2D::new(click.into_iter())
            .with_radii([6.0])
            .with_labels(["click"]),
    )?;

    // 현재 상태 표시 
    let colors = nodes.iter().map(|id| {
        if * id == current {
            [255_u8, 80, 80]
        }
        else {
            [150_u8, 150, 150]
        }
    });

    rec.log(
        "graph",
        &GraphNodes::new(nodes.iter().copied())
            .with_labels(nodes.iter().copied())
            .with_colors(colors),
    )?;

    // 누적해서 발견한 전체 간선 전달
    rec.log(
        "graph",
        &GraphEdges::new(edges.iter().copied())
            .with_directed_edges(),
    )?;

    rec.log(
        "events",
        &TextLog::new(format!("state={current} event={event}")),
    )?;

    Ok(())
}