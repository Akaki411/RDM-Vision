use std::io::{BufReader, Read};
use std::process::{Child, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use crate::error::{AppError, Result};

const IMAGE: &str = "linuxserver/ffmpeg:8.1.2";
static SEQ: AtomicU64 = AtomicU64::new(0);

// Контейнер ffmpeg: тянет RTSP и отдаёт сырые gray-кадры в stdout
pub struct DockerFrameStream
{
    name: String,
    child: Child,
    stdout: BufReader<ChildStdout>,
    stop: Arc<AtomicBool>,
    closed: Arc<AtomicBool>,
    watcher: Option<JoinHandle<()>>
}

impl DockerFrameStream
{
    pub fn spawn(
        camera_url: &str,
        fps: f64,
        transport: &str,
        width: u32,
        height: u32,
        stop: Arc<AtomicBool>
    ) -> Result<Self>
    {
        let n = SEQ.fetch_add(1, Ordering::Relaxed);
        let name = format!("rdm-vision-{}-{}", std::process::id(), n);
        let url = to_container_url(camera_url);

        let mut cmd = Command::new("docker");
        cmd.arg("run")
            .arg("--rm")
            .arg("--name")
            .arg(&name)
            .arg("--add-host=host.docker.internal:host-gateway")
            .arg(IMAGE)
            .arg("-nostdin")
            .arg("-loglevel")
            .arg("error");

        if let Some(t) = transport_flag(transport)
        {
            cmd.arg("-rtsp_transport").arg(t);
        }

        cmd.arg("-i")
            .arg(&url)
            .arg("-an")
            .arg("-vf")
            .arg(format!("scale={width}:{height},format=gray"));

        // Ограничение частоты кадров
        if fps > 0.0
        {
            cmd.arg("-r").arg(format!("{fps}"));
        }

        cmd.arg("-f")
            .arg("rawvideo")
            .arg("-")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());

        let mut child = cmd
            .spawn()
            .map_err(|e| AppError::Camera(format!("failed to start docker ffmpeg: {e}")))?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| AppError::Camera("no stdout from ffmpeg".into()))?;

        let closed = Arc::new(AtomicBool::new(false));
        let watch_stop = stop.clone();
        let watch_closed = closed.clone();
        let watch_name = name.clone();
        let watcher = std::thread::Builder::new()
            .name("ffmpeg-watch".into())
            .spawn(move ||
            {
                while !watch_stop.load(Ordering::Relaxed) && !watch_closed.load(Ordering::Relaxed)
                {
                    std::thread::sleep(Duration::from_millis(100));
                }
                if watch_stop.load(Ordering::Relaxed)
                {
                    kill_container(&watch_name);
                }
            })
            .ok();

        return Ok(Self
        {
            name,
            child,
            stdout: BufReader::new(stdout),
            stop,
            closed,
            watcher
        });
    }

    // Прочитать один кадр фиксированного размера, если None, то поток закончился
    pub fn read_frame(&mut self, expected_bytes: usize) -> Option<Vec<u8>>
    {
        if self.stop.load(Ordering::Relaxed)
        {
            return None;
        }

        let mut buf = vec![0u8; expected_bytes];
        match self.stdout.read_exact(&mut buf)
        {
            Ok(()) => Some(buf),
            Err(_) => None
        }
    }
}

impl Drop for DockerFrameStream
{
    fn drop(&mut self)
    {
        self.closed.store(true, Ordering::Relaxed);
        let _ = self.child.kill();
        kill_container(&self.name);
        if let Some(handle) = self.watcher.take()
        {
            let _ = handle.join();
        }
    }
}

// Из контейнера localhost недоступен, меняем на host.docker.internal (костыль)
fn to_container_url(url: &str) -> String
{
    url.replace("localhost", "host.docker.internal")
        .replace("127.0.0.1", "host.docker.internal")
}

fn transport_flag(name: &str) -> Option<&'static str>
{
    match name
    {
        "tcp" => Some("tcp"),
        "udp" => Some("udp"),
        _ => None
    }
}

// Принудительно удаляем контейнеры чтобы не плодились
fn kill_container(name: &str)
{
    let _ = Command::new("docker")
        .args(["rm", "-f", name])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}
