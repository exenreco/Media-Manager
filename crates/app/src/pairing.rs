use anyhow::{bail, Context, Result};
use base64::Engine;
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Invitation {
    pub version: u8,
    pub url: String,
    pub code: String,
    #[serde(default)]
    pub certificate: String,
    #[serde(default)]
    pub desktop: String,
    #[serde(default)]
    pub mode: String,
}
impl Invitation {
    pub fn parse(text: &str) -> Result<Self> {
        if text.len() > 8192 {
            bail!("Invalid pairing invitation");
        }
        let value: Self = serde_json::from_str(text)
            .context("This QR code is not a Media Manager pairing invitation")?;
        if !matches!(value.version, 1 | 2) || uuid::Uuid::parse_str(&value.code).is_err() {
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
        if value.version == 2 && !matches!(value.mode.as_str(), "local" | "remote") {
            bail!("Unknown pairing connection type");
        }
        if !value.certificate.is_empty() {
            if value.version != 2 || value.mode != "local" {
                bail!("Only local pairing may supply a desktop certificate");
            }
            let ip: std::net::IpAddr = url
                .host_str()
                .unwrap_or_default()
                .trim_matches(['[', ']'])
                .parse()
                .context("Local pairing needs a numeric LAN address")?;
            if !is_local(ip) {
                bail!("Local pairing requires a private network address");
            }
            let der = base64::engine::general_purpose::STANDARD.decode(&value.certificate)?;
            reqwest::Certificate::from_der(&der).context("Invalid desktop certificate")?;
        } else if value.mode == "local" {
            bail!("Local pairing requires its desktop certificate");
        }
        if value.desktop.len() > 100 {
            bail!("Invalid desktop name");
        }
        Ok(value)
    }
}
#[cfg(any(not(any(target_os = "android", target_os = "ios")), test))]
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
#[cfg(test)]
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
pub(crate) fn is_local(ip: std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(ip) => ip.is_private() || ip.is_loopback() || ip.is_link_local(),
        std::net::IpAddr::V6(ip) => {
            ip.is_loopback() || ip.is_unique_local() || ip.is_unicast_link_local()
        }
    }
}
#[cfg(any(target_os = "android", target_os = "ios"))]
pub async fn scan_invitation() -> Result<Invitation> {
    let text = crate::mobile_storage::scan_pairing().await?;
    Invitation::parse(&text)
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
            certificate: String::new(),
            desktop: String::new(),
            mode: String::new(),
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

#[cfg(test)]
mod local_invitation_tests {
    use super::*;
    #[test]
    fn rejects_unpinned_local_and_unexpected_trust() {
        let value = Invitation {
            version: 2,
            url: "https://192.168.1.5:8787".into(),
            code: uuid::Uuid::new_v4().to_string(),
            certificate: String::new(),
            desktop: "Desktop".into(),
            mode: "local".into(),
        };
        assert!(Invitation::parse(&serde_json::to_string(&value).unwrap()).is_err());
        let remote = Invitation {
            mode: "remote".into(),
            certificate: "invalid".into(),
            ..value.clone()
        };
        assert!(Invitation::parse(&serde_json::to_string(&remote).unwrap()).is_err());
        let public = Invitation {
            url: "https://8.8.8.8".into(),
            certificate: "invalid".into(),
            ..value
        };
        assert!(Invitation::parse(&serde_json::to_string(&public).unwrap()).is_err());
    }
}
