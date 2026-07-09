use std::path::PathBuf;
use clap::Parser;
use idm_engine::engine::connection::ConnectionPool;
use idm_engine::engine::scheduler::DownloadScheduler;
use idm_engine::engine::task::TaskState;
use idm_engine::engine::speed;

#[derive(Parser)]
#[command(name = "idm-master", about = "IDM Master - High Performance Download Manager")]
struct Cli {
    /// 下载 URL
    #[arg(short, long)]
    url: String,

    /// 保存目录（默认当前目录）
    #[arg(short, long, default_value = ".")]
    output: PathBuf,

    /// 最大并发连接数（默认 32）
    #[arg(short = 'c', long, default_value = "32")]
    connections: usize,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    println!("╔══════════════════════════════════════╗");
    println!("║       IDM Master v0.1.0              ║");
    println!("╚══════════════════════════════════════╝");
    println!("  URL:      {}", cli.url);
    println!("  Save to:  {}", cli.output.display());
    println!("  Threads:  {}", cli.connections);
    println!("──────────────────────────────────────");

    let pool = ConnectionPool::new(cli.connections);
    let scheduler = DownloadScheduler::new(pool);

    // 注册进度回调
    scheduler.on_progress(|ev| {
        print!("\r  {}  {:5.1}%  {}  {}/{}                    ",
            truncate(&ev.filename, 30),
            ev.progress * 100.0,
            speed::format_bytes_per_sec(ev.speed_bps),
            speed::format_bytes(ev.downloaded),
            speed::format_bytes(ev.total),
        );
    });

    let task_id = scheduler.submit(cli.url, cli.output).await?;
    println!("\n  Task created: {}\n", task_id);

    // 轮询等待完成
    loop {
        let tasks = scheduler.list();
        if let Some((_, _, state, _)) = tasks.iter().find(|(id, ..)| *id == task_id) {
            match state {
                TaskState::Completed => {
                    println!("\n\n  ✓ Download complete!");
                    break;
                }
                TaskState::Error(e) => {
                    println!("\n\n  ✗ Error: {}", e);
                    break;
                }
                TaskState::Cancelled => {
                    println!("\n\n  ✗ Cancelled.");
                    break;
                }
                _ => {}
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }

    Ok(())
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max - 1])
    }
}
