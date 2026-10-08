use slint::wgpu_30::{wgpu, WGPUConfiguration};

pub async fn shared_device() -> Result<WGPUConfiguration, Box<dyn std::error::Error>> {
    // 所有窗口共用一个设备，避免提醒和关于窗口重复创建驱动上下文。
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::DX12,
        flags: wgpu::InstanceFlags::from_build_config(),
        backend_options: wgpu::BackendOptions::default(),
        memory_budget_thresholds: wgpu::MemoryBudgetThresholds::default(),
        display: None,
    });
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions::default())
        .await?;
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("GitHubSP"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::downlevel_webgl2_defaults()
                .using_resolution(adapter.limits()),
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            memory_hints: wgpu::MemoryHints::MemoryUsage,
            trace: wgpu::Trace::default(),
        })
        .await?;
    Ok(WGPUConfiguration::Manual {
        instance,
        adapter,
        device,
        queue,
    })
}
