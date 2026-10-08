use crate::{AboutWindow, MainWindow};
use slint::ComponentHandle;
use std::{cell::RefCell, rc::Rc};

fn pages(text: &str, limit: usize) -> Vec<&str> {
    let mut result = Vec::new();
    let mut start = 0;
    for (count, (index, _)) in text.char_indices().enumerate() {
        if count > 0 && count % limit == 0 {
            result.push(&text[start..index]);
            start = index;
        }
    }
    result.push(&text[start..]);
    result
}

pub(crate) fn bind_licenses(dialog: &AboutWindow) {
    // 声明完整嵌入，但每次仅排版一页，避免打开关于界面时渲染数百万字符。
    let pages = pages(include_str!("../resources/THIRD_PARTY_NOTICES.txt"), 8000);
    dialog.set_license_count(pages.len() as i32);
    dialog.set_licenses(pages[0].into());
    let weak = dialog.as_weak();
    dialog.on_license_page(move |index| {
        if let (Ok(index), Some(dialog)) = (usize::try_from(index), weak.upgrade()) {
            if let Some(page) = pages.get(index) {
                dialog.set_license_index(index as i32);
                dialog.set_licenses((*page).into());
            }
        }
    });
}

pub fn install(main: &MainWindow) {
    let retained = Rc::new(RefCell::new(None::<AboutWindow>));
    let weak = main.as_weak();
    main.on_about(move || {
        let result = || -> Result<(), slint::PlatformError> {
            if retained.borrow().is_none() {
                let dialog = AboutWindow::new()?;
                bind_licenses(&dialog);
                let main_weak = weak.clone();
                let weak = dialog.as_weak();
                dialog.on_dismiss(move || {
                    if let Some(dialog) = weak.upgrade() {
                        if let Err(error) = dialog.hide() {
                            if let Some(main) = main_weak.upgrade() {
                                main.set_message(format!("关闭关于窗口失败：{error}").into());
                            }
                        }
                    }
                });
                retained.replace(Some(dialog));
            }
            if let Some(dialog) = retained.borrow().as_ref() {
                dialog.show()?;
            }
            Ok(())
        };
        if let Err(error) = result() {
            if let Some(main) = weak.upgrade() {
                main.set_message(format!("打开关于窗口失败：{error}").into());
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn license_pages_preserve_all_text_and_utf8_boundaries() {
        let text = "许可证\nCopyright © 示例\n".repeat(1000);
        let result = pages(&text, 8000);
        assert!(result.len() > 1);
        assert!(result.iter().all(|page| page.chars().count() <= 8000));
        assert_eq!(result.concat(), text);
    }
}
