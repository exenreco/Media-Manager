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
pub fn NativeAdBridge() -> Element {
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
        let _ = document::eval(include_str!("fullscreen-ad.js"));
        let _ = document::eval(include_str!("native-ad.js"));
    });
    use_drop(|| {
        let _ = document::eval(
            "window.mediaManagerAdCleanup?.();window.mediaManagerFullscreenCleanup?.()",
        );
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
pub fn NativeAdSlot(placement: &'static str) -> Element {
    rsx! {
        div { class:"native-ad-container", style: "margin:16px 0 24px",
            div { id: "native-ad-slot-{placement}", class: "native-ad-slot", style: "height:440px;min-width:240px;position:relative", "aria-label": "Advertisement" }
            p { class: "hint native-ad-status", role: "status", "Advertisement loads when this section is visible." }
            button { r#type: "button", class: "secondary ad-retry", hidden: true, "Retry ad" }
            button { r#type: "button", class: "secondary ad-privacy", hidden: true, "Ad privacy options" }
        }
    }
}

/// Only native mobile builds offer rewards; entitlement is granted by SDK callbacks.
#[component]
pub fn AdPreferences() -> Element {
    rsx! {
        section {class:"settings-group",
            h2 {"Advertising"}
            p {class:"hint","Watch an optional rewarded ad to hide all ads on this device for 30 minutes. Closing before earning the reward does not activate the benefit."}
            button {r#type:"button",class:"secondary ad-rewarded","Watch an ad — 30 minutes without ads"}
            p {class:"hint fullscreen-ad-status",role:"status","Checking ad availability…"}
            button {r#type:"button",class:"secondary ad-privacy",hidden:true,"Ad privacy options"}
        }
    }
}

pub fn rename_completed() {
    if cfg!(any(target_os = "android", target_os = "ios")) {
        let _ = document::eval("window.mediaManagerRenameAd?.()");
    }
}
