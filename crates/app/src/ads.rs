use dioxus::prelude::*;

#[cfg(target_os = "ios")]
#[manganis::ffi("ios")]
extern "Swift" {
    pub type MediaManagerAds;
    pub fn attach(this: &MediaManagerAds, controller: u64);
    pub fn detach(this: &MediaManagerAds);
}

#[component]
pub fn NativeAdSlot() -> Element {
    #[cfg(target_os = "ios")]
    let native = use_hook(|| {
        use dioxus::mobile::tao::platform::ios::WindowExtIOS;
        let plugin = MediaManagerAds::new().ok();
        if let Some(plugin) = &plugin {
            let controller = dioxus::mobile::window().window.ui_view_controller();
            if let Err(error) = attach(plugin, controller as u64) {
                eprintln!("iOS ad bridge unavailable: {error}");
            }
        }
        std::rc::Rc::new(plugin)
    });
    use_effect(|| {
        let _ = document::eval(include_str!("native-ad.js"));
    });
    use_drop(|| {
        let _ = document::eval("window.mediaManagerAdCleanup?.()");
    });
    #[cfg(target_os = "ios")]
    use_drop(move || {
        if let Some(plugin) = native.as_ref() {
            let _ = detach(plugin);
        }
    });
    rsx! {
        div { style: "margin-bottom:24px",
            div { id: "native-ad-slot", style: "height:440px;min-width:240px;position:relative", "aria-label": "Advertisement" }
            button { id: "ad-privacy", r#type: "button", class: "secondary", hidden: true, "Ad privacy options" }
        }
    }
}
