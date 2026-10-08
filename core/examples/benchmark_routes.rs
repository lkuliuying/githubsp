use githubsp_lib::{
    network::{content_range, Network},
    source::parse_release_url,
};
use sha2::{Digest, Sha256};
use std::time::{Duration, Instant};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let url = std::env::args()
        .nth(1)
        .ok_or("用法：benchmark_routes <公开 Release 附件链接>")?;
    let source = parse_release_url(&url)?;
    let network = Network::production()?;
    let limit = 8 * 1024 * 1024_u64;
    for round in 0..3 {
        let mut routes = network.routes.clone();
        let count = routes.len();
        routes.rotate_left(round % count);
        for route in routes {
            let start = Instant::now();
            let sample = async {
                let mut response = network
                    .client
                    .get(route.url(&source))
                    .header("Accept-Encoding", "identity")
                    .header("Range", format!("bytes=0-{}", limit - 1))
                    .send()
                    .await?;
                if response.status() != reqwest::StatusCode::PARTIAL_CONTENT {
                    return Err("未返回分段响应".into());
                }
                let range = content_range(&response)?;
                if range.start != 0 || range.end != (limit - 1).min(range.total - 1) {
                    return Err("响应范围不匹配".into());
                }
                let mut bytes = 0_u64;
                let mut hasher = Sha256::new();
                while let Some(chunk) = response.chunk().await? {
                    bytes += chunk.len() as u64;
                    if bytes > limit {
                        return Err("响应超出采样上限".into());
                    }
                    hasher.update(&chunk);
                }
                if bytes != range.end + 1 {
                    return Err("采样内容不完整".into());
                }
                Ok::<_, Box<dyn std::error::Error>>((bytes, format!("{:x}", hasher.finalize())))
            };
            let result = tokio::time::timeout(Duration::from_secs(30), sample).await;
            let seconds = start.elapsed().as_secs_f64();
            match result {
                Ok(Ok((bytes, sha256))) => println!(
                    "{}",
                    serde_json::json!({"round":round+1,"route":route.name,"bytes":bytes,"seconds":seconds,"mibPerSecond":bytes as f64 / seconds / 1048576.0,"sha256":sha256})
                ),
                // 不输出底层请求 URL，避免记录 CDN 的临时签名。
                _ => println!(
                    "{}",
                    serde_json::json!({"round":round+1,"route":route.name,"error":"采样失败或超过 30 秒","seconds":seconds})
                ),
            }
        }
    }
    Ok(())
}
