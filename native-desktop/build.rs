fn main() {
    slint_build::compile_with_config(
        "ui/main.slint",
        slint_build::CompilerConfiguration::new()
            .with_style("fluent-light".into())
            .with_debug_info(std::env::var("PROFILE").as_deref() == Ok("debug"))
            .embed_resources(slint_build::EmbedResourcesKind::EmbedFiles),
    )
    .expect("无法编译原生界面");
}
