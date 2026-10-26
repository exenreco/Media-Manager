use anyhow::{bail, Context, Result};
use base64::Engine;
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Invitation {
    pub version: u8,
    pub url: String,
    pub code: String,
}
impl Invitation {
    pub fn parse(text: &str) -> Result<Self> {
        if text.len() > 4096 {
            bail!("Invalid pairing invitation");
        }
        let value: Self = serde_json::from_str(text)
            .context("This QR code is not a Media Manager pairing invitation")?;
        if value.version != 1 || uuid::Uuid::parse_str(&value.code).is_err() {
            bail!("Invalid pairing invitation");
        }
        let url = reqwest::Url::parse(&value.url)?;
        if url.scheme() != "https"
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
        {
            bail!("Pairing requires an HTTPS server URL without credentials or query parameters");
        }
        Ok(value)
    }
}
pub fn qr_uri(text: &str) -> Result<String> {
    let code = qrcode::QrCode::with_error_correction_level(text, qrcode::EcLevel::M)?;
    let size = code.width();
    let total = size + 8;
    let mut svg = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {total} {total}" shape-rendering="crispEdges"><rect width="100%" height="100%" fill="#fff"/><path fill="#000" d=""##
    );
    for y in 0..size {
        for x in 0..size {
            if code[(x, y)] == qrcode::Color::Dark {
                svg.push_str(&format!("M{} {}h1v1h-1z", x + 4, y + 4));
            }
        }
    }
    svg.push_str("\"/></svg>");
    Ok(format!(
        "data:image/svg+xml;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(svg)
    ))
}
pub fn decode_image(bytes: &[u8]) -> Result<Invitation> {
    if bytes.len() > 10 * 1024 * 1024 {
        bail!("QR image must be smaller than 10 MB");
    }
    let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes)).with_guessed_format()?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(4096);
    limits.max_image_height = Some(4096);
    limits.max_alloc = Some(128 * 1024 * 1024);
    reader.limits(limits);
    let image = reader.decode()?.to_luma8();
    let mut prepared = rqrr::PreparedImage::prepare(image);
    for grid in prepared.detect_grids() {
        if let Ok((_, text)) = grid.decode() {
            if let Ok(invitation) = Invitation::parse(&text) {
                return Ok(invitation);
            }
        }
    }
    bail!("No Media Manager pairing QR was found in this image")
}
pub async fn pick_invitation() -> Result<Invitation> {
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        let bytes = crate::mobile_storage::pick_image().await?;
        decode_image(&bytes)
    }
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        let parent = dioxus::desktop::window();
        let file = rfd::AsyncFileDialog::new()
            .set_parent(parent.window.as_ref())
            .add_filter("QR image", &["png", "jpg", "jpeg"])
            .pick_file()
            .await
            .context("Image selection cancelled")?;
        if std::fs::metadata(file.path())?.len() > 10 * 1024 * 1024 {
            bail!("QR image must be smaller than 10 MB");
        }
        let bytes = file.read().await;
        decode_image(&bytes)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invitations_require_https_and_no_embedded_credentials() {
        let code = uuid::Uuid::nil().to_string();
        for url in [
            "http://example.com",
            "https://user:secret@example.com",
            "https://example.com?token=secret",
        ] {
            assert!(Invitation::parse(
                &serde_json::json!({"version":1,"url":url,"code":code}).to_string()
            )
            .is_err());
        }
        let valid =
            serde_json::json!({"version":1,"url":"https://example.com","code":code}).to_string();
        assert!(Invitation::parse(&valid).is_ok());
        assert!(qr_uri(&valid)
            .unwrap()
            .starts_with("data:image/svg+xml;base64,"));
    }
}

#[cfg(test)]
mod image_tests {
    use super::*;
    #[test]
    fn generated_qr_can_be_decoded_and_reviewed_before_pairing() {
        let invitation = Invitation {
            version: 1,
            url: "https://media.example.com".into(),
            code: uuid::Uuid::nil().to_string(),
        };
        let text = serde_json::to_string(&invitation).unwrap();
        let code = qrcode::QrCode::new(text).unwrap();
        let scale = 6;
        let size = (code.width() + 8) * scale;
        let pixels = image::GrayImage::from_fn(size as u32, size as u32, |x, y| {
            let x = x as usize / scale;
            let y = y as usize / scale;
            image::Luma([
                if x >= 4
                    && y >= 4
                    && x < code.width() + 4
                    && y < code.width() + 4
                    && code[(x - 4, y - 4)] == qrcode::Color::Dark
                {
                    0
                } else {
                    255
                },
            ])
        });
        let mut bytes = std::io::Cursor::new(vec![]);
        image::DynamicImage::ImageLuma8(pixels)
            .write_to(&mut bytes, image::ImageFormat::Png)
            .unwrap();
        let decoded = decode_image(bytes.get_ref()).unwrap();
        assert_eq!(decoded.url, invitation.url);
        assert_eq!(decoded.code, invitation.code);
    }
}
