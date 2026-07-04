use rdm_vision::config::Settings;
use rdm_vision::pipeline::Pipeline;
use rdm_vision::service::camera::Cameras;

#[tokio::main]
async fn main() -> anyhow::Result<()>
{
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    // Путь к конфигу
    let config_path = std::env::args().nth(1).unwrap_or_else(|| "config.json".into());
    let (settings, created) = Settings::load(&config_path)?;

    // Конфиг только что создан — перезапускаемся, чтобы стартовать с чистого файла
    if created
    {
        tracing::info!(path = %config_path, "config created, restarting");
        let exe = std::env::current_exe()?;
        let args: Vec<String> = std::env::args().skip(1).collect();
        let code = std::process::Command::new(exe).args(args).status()?.code().unwrap_or(0);
        std::process::exit(code);
    }

    tracing::info!(path = %config_path, cameras = settings.cameras.len(), "config loaded");

    // Запуск камер: каждая в своём потоке, кадры идут в общий канал
    let cameras = Cameras::from_settings(&settings)?;
    let (frames, handle) = cameras.spawn(settings.pipeline.channel_capacity);

    // Пайплайн работает до Ctrl-C или закрытия канала
    let pipeline = Pipeline::new(&settings)?;
    tokio::select!
    {
        result = pipeline.run(frames) =>
        {
            if let Err(err) = result
            {
                tracing::error!(error = %err, "pipeline error");
            }
        }
        _ = tokio::signal::ctrl_c() =>
        {
            tracing::info!("shutdown signal received");
        }
    }

    // Остановить камеры
    handle.shutdown();
    tracing::info!("done");
    return Ok(());
}
