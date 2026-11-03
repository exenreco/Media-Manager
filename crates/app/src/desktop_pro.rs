use anyhow::{Context, Result};
use dioxus::prelude::*;
use serde::Deserialize;

#[derive(Clone, Default, Deserialize)]
pub struct StoreState {
    pub owned: bool,
    pub available: bool,
    pub price: String,
    pub message: String,
}

#[cfg(target_os = "macos")]
mod native {
    use super::*;
    use std::{
        ffi::{c_char, CStr},
        sync::Mutex,
    };
    static PENDING: std::sync::LazyLock<
        Mutex<std::collections::HashMap<u64, tokio::sync::oneshot::Sender<StoreState>>>,
    > = std::sync::LazyLock::new(|| Mutex::new(std::collections::HashMap::new()));
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    static SERIAL: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
    unsafe extern "C" {
        fn media_manager_pro_request(
            action: i32,
            request_id: u64,
            callback: extern "C" fn(u64, *const c_char),
        );
    }
    extern "C" fn callback(request_id: u64, json: *const c_char) {
        if json.is_null() {
            return;
        }
        let state = serde_json::from_slice(unsafe { CStr::from_ptr(json) }.to_bytes())
            .unwrap_or_else(|_| StoreState {
                message: "Invalid App Store response".into(),
                ..Default::default()
            });
        if let Some(sender) = PENDING.lock().unwrap().remove(&request_id) {
            let _ = sender.send(state);
        }
    }
    pub async fn request(action: i32) -> Result<StoreState> {
        let _guard = SERIAL.lock().await;
        let (sender, receiver) = tokio::sync::oneshot::channel();
        let request_id = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        PENDING.lock().unwrap().insert(request_id, sender);
        unsafe {
            media_manager_pro_request(action, request_id, callback);
        }
        receiver.await.context("App Store request interrupted")
    }
}

#[cfg(windows)]
mod native {
    use super::*;
    use windows::{
        core::{Interface, HSTRING},
        Services::Store::{StoreContext, StorePurchaseStatus},
        Win32::{Foundation::HWND, UI::Shell::IInitializeWithWindow},
    };
    static WINDOW: std::sync::atomic::AtomicIsize = std::sync::atomic::AtomicIsize::new(0);
    pub fn initialize() {
        use dioxus::desktop::tao::platform::windows::WindowExtWindows;
        WINDOW.store(
            dioxus::desktop::window().hwnd(),
            std::sync::atomic::Ordering::Release,
        );
    }
    async fn entitlement(context: &StoreContext) -> Result<bool> {
        let license = context.GetAppLicenseAsync()?.await?;
        for entry in license.AddOnLicenses()? {
            let value = entry.Value()?;
            if value.InAppOfferToken()? == "desktop_pro" && value.IsActive()? {
                return Ok(true);
            }
        }
        Ok(false)
    }
    pub async fn request(action: i32) -> Result<StoreState> {
        let context = StoreContext::GetDefault()
            .context("Install Media Manager from Microsoft Store to use Pro")?;
        let handle = WINDOW.load(std::sync::atomic::Ordering::Acquire);
        anyhow::ensure!(handle != 0, "Store window is not ready");
        unsafe {
            context
                .cast::<IInitializeWithWindow>()?
                .Initialize(HWND(handle as *mut _))?;
        }
        let kinds = windows_collections::IIterable::<HSTRING>::from(vec![HSTRING::from("Durable")]);
        let owned_before = entitlement(&context).await?;
        let query = async {
            let query = context.GetAssociatedStoreProductsAsync(&kinds)?.await?;
            query.ExtendedError()?.ok()?;
            Ok::<_, anyhow::Error>(query)
        }
        .await;
        let query = match query {
            Ok(query) => query,
            Err(error) => {
                return Ok(StoreState {
                    owned: owned_before,
                    message: if owned_before {
                        "Pro is active. Store pricing is temporarily unavailable.".into()
                    } else {
                        format!("Microsoft Store pricing is unavailable: {error:#}")
                    },
                    ..Default::default()
                })
            }
        };
        let mut product = None;
        for entry in query.Products()? {
            let value = entry.Value()?;
            if value.InAppOfferToken()? == "desktop_pro" {
                product = Some(value);
                break;
            }
        }
        let mut message = String::new();
        if action == 1 {
            if let Some(product) = &product {
                let result = context.RequestPurchaseAsync(&product.StoreId()?)?.await?;
                result.ExtendedError()?.ok()?;
                message = match result.Status()? {
                    StorePurchaseStatus::Succeeded | StorePurchaseStatus::AlreadyPurchased => {
                        "Purchase verified by Microsoft Store."
                    }
                    StorePurchaseStatus::NotPurchased => "Purchase cancelled or not completed.",
                    _ => "Microsoft Store could not complete the purchase.",
                }
                .into();
            }
        }
        let owned = entitlement(&context).await?;
        Ok(StoreState {
            owned,
            available: product.is_some(),
            price: product
                .as_ref()
                .map(|p| p.Price()?.FormattedPrice())
                .transpose()?
                .map(|p| p.to_string())
                .unwrap_or_default(),
            message: if !message.is_empty() {
                message
            } else if owned {
                "Pro is active. Unlimited titles.".into()
            } else if product.is_some() {
                "One-time purchase. Unlimited titles.".into()
            } else {
                "Pro requires a published Microsoft Store product. It is not available for this build yet.".into()
            },
        })
    }
}

pub async fn request(action: i32) -> Result<StoreState> {
    native::request(action).await
}

pub fn initialize() {
    #[cfg(windows)]
    native::initialize();
}

/// Query the platform store for every rename; UI state never authorizes Pro.
pub async fn owned() -> bool {
    request(0).await.is_ok_and(|state| state.owned)
}

#[component]
pub fn ProPanel() -> Element {
    let mut state = use_signal(StoreState::default);
    let mut busy = use_signal(|| true);
    let mut usage = use_signal(String::new);
    use_future(move || async move {
        loop {
            if let Ok(text) = tokio::task::spawn_blocking(crate::rename_quota::summary).await {
                usage.set(text.unwrap_or_else(|e| format!("Usage status unavailable: {e:#}")));
            }
            tokio::time::sleep(std::time::Duration::from_secs(30)).await;
        }
    });
    use_future(move || async move {
        state.set(request(0).await.unwrap_or_else(|e| StoreState {
            message: format!("Store unavailable: {e:#}"),
            ..Default::default()
        }));
        busy.set(false);
    });
    let mut run = move |action| {
        if busy() {
            return;
        }
        busy.set(true);
        spawn(async move {
            state.set(request(action).await.unwrap_or_else(|e| StoreState {
                message: format!("Store unavailable: {e:#}"),
                ..Default::default()
            }));
            busy.set(false);
        });
    };
    let current = state();
    rsx! {section {class:"settings-group pro-settings",
        h2 {if current.owned {"Media Manager Pro"} else {"Go Pro"}}
        p {class:"hint","Pro unlocks unlimited movie and TV show renames. One-time purchase; no subscription."}
        if !current.owned {
            p {class:"hint","Free: rename two distinct movie or TV show titles, then wait two hours. Episodes of one show count as one title."}
            p {class:"status",role:"status","{usage}"}
        }
        div {class:"pro-actions",
            button {class:"primary",disabled:busy() || current.owned,onclick:move |_|run(1),
                if current.owned {"Pro active"} else if current.price.is_empty() {"Go Pro"} else {"Go Pro · {current.price}"}}
            button {class:"secondary",disabled:busy(),onclick:move |_|run(2),"Restore Pro purchase"}
        }
        if busy() {p {class:"hint",role:"status","Checking the store…"}}
        if !current.available && !current.owned && !busy() {p {class:"hint","Purchasing is not available for this build yet."}}
        p {class:"status",role:"status","{current.message}"}
    }}
}
