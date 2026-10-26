use dioxus::prelude::*;

#[cfg(target_os = "ios")]
#[manganis::ffi("ios")]
extern "Swift" {
    pub type MediaManagerAds;
    pub fn attach(this: &MediaManagerAds, controller: u64);
    pub fn detach(this: &MediaManagerAds);
    pub type MediaManagerStorage;
    pub fn attach_storage(this: &MediaManagerStorage, controller: u64);
    pub fn detach_storage(this: &MediaManagerStorage);
}

#[component]
fn NativeAdBridge() -> Element {
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
    rsx! {}
}

#[component]
pub fn NativeAdSlot(#[props(default = false)] posters: bool) -> Element {
    let suffix = if posters { "posters" } else { "library" };
    rsx! {
        if !posters { NativeAdBridge {} }
        div { style: "margin-bottom:24px",
            div { id: "native-ad-slot-{suffix}", class: "native-ad-slot", style: "height:440px;min-width:240px;position:relative", "aria-label": "Advertisement" }
            p { class: "hint native-ad-status", role: "status", "Advertisement loads when this section is visible." }
            button { r#type: "button", class: "secondary ad-retry", hidden: true, "Retry ad" }
            button { r#type: "button", class: "secondary ad-privacy", hidden: true, "Ad privacy options" }
        }
    }
}
