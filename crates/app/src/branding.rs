use base64::{engine::general_purpose::STANDARD, Engine};
use dioxus::prelude::*;
use std::sync::OnceLock;

const SPLASH_CSS: &str = include_str!("splash.css");

pub const ICON: &[u8] = include_bytes!("../../../assets/branding/app-icon.png");
pub const SPLASH: &[u8] = include_bytes!("../../../assets/branding/splash.png");

pub fn icon_uri() -> &'static str {
    static URI: OnceLock<String> = OnceLock::new();
    URI.get_or_init(|| format!("data:image/png;base64,{}", STANDARD.encode(ICON)))
}

fn splash_uri() -> &'static str {
    static URI: OnceLock<String> = OnceLock::new();
    URI.get_or_init(|| format!("data:image/png;base64,{}", STANDARD.encode(SPLASH)))
}

#[cfg(all(
    feature = "desktop",
    not(any(target_os = "android", target_os = "ios"))
))]
pub fn desktop_config() -> dioxus::desktop::Config {
    use dioxus::desktop::{tao::window::Icon, Config, LogicalSize, WindowBuilder};
    let icon = image::load_from_memory(ICON).ok().and_then(|image| {
        let rgba = image
            .resize_exact(64, 64, image::imageops::FilterType::Lanczos3)
            .into_rgba8();
        let (width, height) = rgba.dimensions();
        Icon::from_rgba(rgba.into_raw(), width, height).ok()
    });
    let window = WindowBuilder::new()
        .with_title("Media Manager")
        .with_window_icon(icon)
        .with_transparent(true)
        .with_background_color((0, 0, 0, 0))
        .with_decorations(false)
        .with_resizable(false)
        .with_inner_size(LogicalSize::new(860.0, 540.0));
    #[cfg(windows)]
    let window = {
        use dioxus::desktop::tao::platform::windows::WindowBuilderExtWindows;
        // Do not let Windows composite an opaque redirection surface behind
        // the transparent WebView. CSS and WebView alpha alone cannot clear it.
        window
            .with_no_redirection_bitmap(true)
            .with_undecorated_shadow(false)
    };
    Config::new()
        // Apply before the first WebView paint, rather than after Dioxus mounts.
        .with_custom_index(include_str!("index.html").to_owned())
        .with_window(window)
        .with_background_color((0, 0, 0, 0))
        .with_on_window(|window, _| {
            #[cfg(windows)]
            set_splash_border(window.as_ref(), true);
            center_window(window.as_ref());
        })
}

#[cfg(all(feature = "desktop", windows))]
fn set_splash_border(window: &dioxus::desktop::tao::window::Window, splash: bool) {
    use dioxus::desktop::tao::platform::windows::WindowExtWindows;
    use windows_sys::Win32::Graphics::Dwm::{
        DwmSetWindowAttribute, DWMWA_BORDER_COLOR, DWMWA_COLOR_DEFAULT, DWMWA_COLOR_NONE,
        DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_DEFAULT, DWMWCP_DONOTROUND,
    };
    let border = if splash {
        DWMWA_COLOR_NONE
    } else {
        DWMWA_COLOR_DEFAULT
    };
    let corners = if splash {
        DWMWCP_DONOTROUND
    } else {
        DWMWCP_DEFAULT
    };
    // Windows 11 draws a compositor border even on some undecorated windows.
    // Unsupported attributes on older Windows are harmless; keep the normal
    // dashboard frame once the splash is dismissed.
    unsafe {
        let hwnd = window.hwnd() as windows_sys::Win32::Foundation::HWND;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_BORDER_COLOR as u32,
            (&border as *const u32).cast(),
            std::mem::size_of_val(&border) as u32,
        );
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE as u32,
            (&corners as *const i32).cast(),
            std::mem::size_of_val(&corners) as u32,
        );
    }
}

#[cfg(all(
    feature = "desktop",
    not(any(target_os = "android", target_os = "ios"))
))]
fn center_window(window: &dioxus::desktop::tao::window::Window) {
    use dioxus::desktop::tao::dpi::PhysicalPosition;
    if let Some(monitor) = window.current_monitor() {
        let origin = monitor.position();
        let size = monitor.size();
        let window_size = window.outer_size();
        window.set_outer_position(PhysicalPosition::new(
            origin.x + (size.width.saturating_sub(window_size.width) / 2) as i32,
            origin.y + (size.height.saturating_sub(window_size.height) / 2) as i32,
        ));
    }
}

pub fn finish_splash() {
    #[cfg(all(
        feature = "desktop",
        not(any(target_os = "android", target_os = "ios"))
    ))]
    {
        use dioxus::desktop::LogicalSize;
        let window = dioxus::desktop::window();
        let size = window
            .current_monitor()
            .map(|m| m.size().to_logical::<f64>(window.scale_factor()));
        let (width, height) = size.map_or((1280.0, 840.0), |s| {
            (
                (s.width - 64.0).clamp(480.0, 1280.0),
                (s.height - 100.0).clamp(400.0, 840.0),
            )
        });
        // Tao's macOS decoration update queues a style mask using the current
        // resizable state. Set it first so that queued update preserves resizing.
        window.set_resizable(true);
        window.set_decorations(true);
        #[cfg(windows)]
        set_splash_border(window.window.as_ref(), false);
        window.set_inner_size(LogicalSize::new(width, height));
        center_window(window.window.as_ref());
    }
}

#[component]
pub fn Splash(
    progress: u8,
    onartworkready: EventHandler<()>,
    oncontinue: EventHandler<()>,
) -> Element {
    let progress = progress.min(100);
    let status = match progress {
        0..=69 => "Loading saved workspace…",
        70..=99 => "Preparing artwork…",
        _ => "Ready",
    };
    rsx! {
        style { "{SPLASH_CSS}" }
        div { class: "splash-stage",
            div { class: "splash-artwork",
                img { src: splash_uri(), alt: "Media Manager — Your library. In perfect order.", draggable: "false",
                    onload: move |_| onartworkready.call(()), onerror: move |_| onartworkready.call(()) }
                div { class: "splash-loading",
                    div { class: "splash-progress", role: "progressbar", "aria-label": "Starting Media Manager",
                        "aria-valuemin": "0", "aria-valuemax": "100", "aria-valuenow": progress.to_string(), "aria-valuetext": status,
                        div { class: "splash-progress-fill", style: "width:{progress}%" }
                    }
                    div { class: "splash-progress-status", role: "status", "{status} {progress}%" }
                }
            }
            button { class: "splash-skip", disabled: progress < 100, onclick: move |_| oncontinue.call(()), "Open library →" }
        }
    }
}
