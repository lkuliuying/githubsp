use crate::{
    bridge::{self, Area, Epoch},
    service::Service,
    MainWindow, Preferences, Workspace,
};
use githubsp_lib::model::Settings;
use slint::ComponentHandle;
use std::sync::Arc;

pub fn megabytes(kib: u32) -> String {
    let value = format!("{:.6}", kib as f64 * 1024.0 / 1_000_000.0);
    value.trim_end_matches('0').trim_end_matches('.').to_owned()
}

pub fn parse_limit(input: &str, original: u32) -> Result<u32, String> {
    let error = "请输入 0（不限速）或 0.001024 至 10240 MB/s 之间的数值";
    let value = input.trim().parse::<f64>().map_err(|_| error)?;
    if !value.is_finite() || (value != 0.0 && !(0.001024..=10240.0).contains(&value)) {
        return Err(error.into());
    }
    if megabytes(original).parse::<f64>().ok() == Some(value) {
        return Ok(original);
    }
    Ok((value * 1_000_000.0 / 1024.0).round() as u32)
}

fn apply(window: &MainWindow, settings: &Settings) {
    let ui = window.global::<Preferences>();
    ui.set_limit(megabytes(settings.limit_kib).into());
    ui.set_unlimited(settings.limit_kib == 0);
    ui.set_slider((settings.limit_kib as f64 * 1024.0 / 1_000_000.0).clamp(0.0, 100.0) as f32);
    ui.set_close_to_tray(settings.close_to_tray);
    ui.set_completion(settings.background_completion_notice);
    ui.set_auto_check(settings.auto_check);
}

pub fn has_unsaved(window: &MainWindow, service: &Service) -> bool {
    let ui = window.global::<Preferences>();
    let state = service.state.borrow();
    let Some(snapshot) = state.as_ref() else {
        return true;
    };
    let saved = &snapshot.settings;
    ui.get_busy()
        || parse_limit(&ui.get_limit(), saved.limit_kib).ok() != Some(saved.limit_kib)
        || ui.get_close_to_tray() != saved.close_to_tray
        || ui.get_completion() != saved.background_completion_notice
        || ui.get_auto_check() != saved.auto_check
}

pub fn bind(window: &MainWindow, api: &Arc<Service>, runtime: &tokio::runtime::Handle) {
    if let Some(snapshot) = api.state.borrow().clone() {
        apply(window, &snapshot.settings);
    }
    let weak = window.as_weak();
    window.global::<Preferences>().on_edited(move || {
        if let Some(window) = weak.upgrade() {
            let ui = window.global::<Preferences>();
            let value = ui
                .get_limit()
                .trim()
                .parse::<f64>()
                .ok()
                .filter(|v| v.is_finite());
            ui.set_unlimited(value == Some(0.0));
            ui.set_slider(value.unwrap_or(0.0).clamp(0.0, 100.0) as f32);
            ui.set_saved(false);
            ui.set_message("有尚未保存的设置".into());
        }
    });
    let weak = window.as_weak();
    window
        .global::<Preferences>()
        .on_slider_edited(move |value| {
            if let Some(window) = weak.upgrade() {
                let ui = window.global::<Preferences>();
                ui.set_limit(format!("{}", (value * 4.0).round() / 4.0).into());
                ui.invoke_edited();
            }
        });
    let (weak, previous) = (window.as_weak(), std::cell::Cell::new(1.048576_f64));
    window.global::<Preferences>().on_toggle_limit(move || {
        if let Some(window) = weak.upgrade() {
            let ui = window.global::<Preferences>();
            let value = ui.get_limit().trim().parse::<f64>().unwrap_or(0.0);
            if value == 0.0 {
                ui.set_limit(previous.get().to_string().into());
            } else {
                if value.is_finite() && value > 0.0 {
                    previous.set(value);
                }
                ui.set_limit("0".into());
            }
            ui.invoke_edited();
        }
    });
    let weak = window.as_weak();
    window.global::<Preferences>().on_reset(move || {
        if let Some(window) = weak.upgrade() {
            apply(&window, &Settings::default());
            let ui = window.global::<Preferences>();
            ui.set_saved(false);
            ui.set_message("已恢复默认值，保存设置后生效。".into());
        }
    });
    let (weak, api, runtime, epoch) = (
        window.as_weak(),
        api.clone(),
        runtime.clone(),
        Epoch::default(),
    );
    window.global::<Preferences>().on_save(move || {
        let Some(window) = weak.upgrade() else {
            return;
        };
        let ui = window.global::<Preferences>();
        let original = api
            .state
            .borrow()
            .as_ref()
            .map(|s| s.settings.limit_kib)
            .unwrap_or(0);
        let limit_kib = match parse_limit(&ui.get_limit(), original) {
            Ok(value) => value,
            Err(error) => {
                window
                    .global::<Workspace>()
                    .invoke_select_settings_section(0);
                ui.set_message(error.into());
                return;
            }
        };
        let settings = Settings {
            limit_kib,
            close_to_tray: ui.get_close_to_tray(),
            auto_check: ui.get_auto_check(),
            background_completion_notice: ui.get_completion(),
        };
        ui.set_saved(false);
        let manager = api.manager.clone();
        bridge::run(
            &window,
            &api,
            &runtime,
            Area::Preferences,
            epoch.ticket(),
            async move { manager.settings(settings).await.map_err(|e| e.to_string()) },
            |window, snapshot| {
                apply(window, &snapshot.settings);
                let ui = window.global::<Preferences>();
                ui.set_saved(true);
                ui.set_message("设置已保存".into());
            },
        );
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rate_conversion_preserves_existing_integer_and_validates_boundaries() {
        for value in [0, 1, 256, 1024, 999_999, 10_000_000] {
            assert_eq!(parse_limit(&megabytes(value), value).unwrap(), value);
        }
        assert_eq!(parse_limit("0.001024", 0).unwrap(), 1);
        assert_eq!(parse_limit("10240", 0).unwrap(), 10_000_000);
        for value in ["", " ", "NaN", "inf", "-1", "0.001", "10240.1", "word"] {
            assert!(parse_limit(value, 0).is_err(), "{value}");
        }
    }
}
