//! Proje CRM masaüstü kabuğu.
//!
//! Mobil kabukla aynı felsefe: uygulama = native kabuk + panelin kendisi (web
//! görüntüleyici). Native olan yalnız:
//!  - panel adresi seçimi (yerel ekran) ve kalıcı saklama,
//!  - panel dışı bağlantıların (tel:, mailto:, wa.me, harici siteler) sistem
//!    uygulamasına yönlendirilmesi,
//!  - indirmelerin İndirilenler klasörüne alınması,
//!  - okunmamış bildirimlerin masaüstü bildirimi olarak gösterilmesi,
//!  - uygulama menüsü (panel adresini değiştir, yenile).
//! Görünen her şey web panelidir; tasarım burada değişmez.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::path::PathBuf;
use std::time::Duration;

use serde::Deserialize;
use tauri::menu::{Menu, MenuItem, Submenu};
use tauri::webview::DownloadEvent;
use tauri::{AppHandle, Manager, Runtime, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_notification::NotificationExt;
use tauri_plugin_opener::OpenerExt;
use tauri_plugin_store::StoreExt;
use url::Url;

const AYAR_DOSYASI: &str = "ayarlar.json";
const PANEL_ANAHTARI: &str = "panelUrl";
const PENCERE: &str = "main";
/// Bildirim yoklama aralığı (panelin canlı yenilemesiyle uyumlu, hafif).
const BILDIRIM_ARALIGI: Duration = Duration::from_secs(60);

// ---------------------------------------------------------------- panel adresi

/// "demo.projecrm.com", "https://x.y" ya da "localhost:3005" → origin
/// (mobil kabuktaki panelOrigin ile aynı kural). Geçersizse None.
fn panel_origin(girdi: &str) -> Option<String> {
    let temiz = girdi.trim().trim_end_matches('/');
    if temiz.is_empty() {
        return None;
    }
    let tam = if temiz.starts_with("http://") || temiz.starts_with("https://") {
        temiz.to_string()
    } else if temiz.starts_with("localhost") || temiz.starts_with("127.0.0.1") || temiz.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
        format!("http://{temiz}")
    } else {
        format!("https://{temiz}")
    };
    let url = Url::parse(&tam).ok()?;
    if url.host_str().is_none() || url.path() != "/" || url.query().is_some() {
        return None;
    }
    Some(url.origin().ascii_serialization())
}

fn panel_oku_store<R: Runtime>(app: &AppHandle<R>) -> Option<String> {
    let store = app.store(AYAR_DOSYASI).ok()?;
    store.get(PANEL_ANAHTARI).and_then(|v| v.as_str().map(|s| s.to_string()))
}

fn panel_yaz_store<R: Runtime>(app: &AppHandle<R>, origin: Option<&str>) -> Result<(), String> {
    let store = app.store(AYAR_DOSYASI).map_err(|e| e.to_string())?;
    match origin {
        Some(o) => store.set(PANEL_ANAHTARI, serde_json::json!(o)),
        None => {
            store.delete(PANEL_ANAHTARI);
        }
    }
    store.save().map_err(|e| e.to_string())
}

fn panele_git<R: Runtime>(app: &AppHandle<R>, origin: &str) -> Result<(), String> {
    let pencere = app.get_webview_window(PENCERE).ok_or("pencere yok")?;
    let url = Url::parse(&format!("{origin}/")).map_err(|e| e.to_string())?;
    pencere.navigate(url).map_err(|e| e.to_string())
}

fn yerel_ekrana_git<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    let pencere = app.get_webview_window(PENCERE).ok_or("pencere yok")?;
    let url = WebviewUrl::App("index.html".into());
    let url: Url = match url {
        WebviewUrl::App(p) => {
            // Üretimde tauri://localhost (macOS) / http://tauri.localhost (Windows); geliştirmede devUrl.
            let taban = app.config().build.dev_url.clone().filter(|_| cfg!(debug_assertions))
                .map(|u| u.to_string())
                .unwrap_or_else(|| if cfg!(windows) { "http://tauri.localhost/".into() } else { "tauri://localhost/".into() });
            Url::parse(&taban).and_then(|b| b.join(&p.to_string_lossy())).map_err(|e| e.to_string())?
        }
        _ => unreachable!(),
    };
    pencere.navigate(url).map_err(|e| e.to_string())
}

#[tauri::command]
fn panel_oku(app: AppHandle) -> Option<String> {
    panel_oku_store(&app)
}

/// Adresi doğrular, saklar ve pencereyi panele götürür. Hata metni Türkçe (UI'da gösterilir).
#[tauri::command]
fn panel_ayarla(app: AppHandle, adres: String) -> Result<String, String> {
    let origin = panel_origin(&adres).ok_or_else(|| "Geçerli bir panel adresi yazın (örn. demo.projecrm.com).".to_string())?;
    panel_yaz_store(&app, Some(&origin))?;
    panele_git(&app, &origin)?;
    Ok(origin)
}

// ---------------------------------------------------------------- gezinme / indirme

/// Panel dışı her şey sistem uygulamasına gider; kabuk yalnız kendi yerel ekranını
/// ve seçili panel origin'ini yükler.
fn gezinmeye_izin<R: Runtime>(app: &AppHandle<R>, url: &Url) -> bool {
    match url.scheme() {
        "tauri" | "about" | "blob" | "data" => return true,
        "http" | "https" => {
            if url.host_str() == Some("tauri.localhost") {
                return true;
            }
            if cfg!(debug_assertions) && url.host_str() == Some("localhost") && url.port() == Some(1420) {
                return true;
            }
            if let Some(origin) = panel_oku_store(app) {
                if url.origin().ascii_serialization() == origin {
                    return true;
                }
            }
        }
        _ => {}
    }
    // tel:, mailto:, wa.me, harici site → sistem.
    let _ = app.opener().open_url(url.as_str(), None::<&str>);
    false
}

/// İndirmeleri İndirilenler klasörüne alır; aynı ad varsa (2), (3)… ekler.
fn indirme_hedefi<R: Runtime>(app: &AppHandle<R>, url: &Url, onerilen: &PathBuf) -> PathBuf {
    let klasor = app.path().download_dir().unwrap_or_else(|_| std::env::temp_dir());
    let mut ad = onerilen.file_name().map(|s| s.to_string_lossy().to_string()).filter(|s| !s.is_empty())
        .or_else(|| url.path_segments().and_then(|s| s.last()).map(|s| percent_decode(s)))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "dosya".to_string());
    ad = ad.replace(['/', '\\', ':'], "_");
    let mut hedef = klasor.join(&ad);
    let (govde, uzanti) = match ad.rsplit_once('.') {
        Some((g, u)) if !g.is_empty() => (g.to_string(), format!(".{u}")),
        _ => (ad.clone(), String::new()),
    };
    let mut n = 2;
    while hedef.exists() {
        hedef = klasor.join(format!("{govde} ({n}){uzanti}"));
        n += 1;
    }
    hedef
}

fn percent_decode(s: &str) -> String {
    let mut out = Vec::with_capacity(s.len());
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(h) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(h);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

// ---------------------------------------------------------------- bildirim yoklayıcı

#[derive(Deserialize)]
struct BildirimYaniti {
    bildirimler: Vec<Bildirim>,
}

#[derive(Deserialize)]
struct Bildirim {
    id: String,
    mesaj: String,
}

/// Panel oturumunun çerezleriyle okunmamış bildirimleri çeker (çerez + X-ProjeCRM-Mobil
/// başlığı; mobil kabuğun push kaydıyla aynı kimlik yolu). İlk turda yalnız mevcutlar
/// öğrenilir, sonra yeni gelen her bildirim masaüstü bildirimi olur.
async fn bildirim_yokla<R: Runtime>(app: AppHandle<R>) {
    let istemci = reqwest::Client::builder().timeout(Duration::from_secs(15)).build().ok();
    let Some(istemci) = istemci else { return };
    let mut gorulen: HashSet<String> = HashSet::new();
    let mut ilk = true;
    loop {
        tokio::time::sleep(BILDIRIM_ARALIGI).await;
        let Some(origin) = panel_oku_store(&app) else { continue };
        let Some(pencere) = app.get_webview_window(PENCERE) else { continue };
        let Ok(origin_url) = Url::parse(&format!("{origin}/")) else { continue };
        let cerez = match pencere.cookies_for_url(origin_url.clone()) {
            Ok(c) if !c.is_empty() => c.iter().map(|k| format!("{}={}", k.name(), k.value())).collect::<Vec<_>>().join("; "),
            _ => continue, // oturum yok
        };
        let yanit = istemci
            .get(format!("{origin}/api/mobil/v1/bildirimler?yalnizOkunmamis=1&adet=20"))
            .header("Cookie", cerez)
            .header("X-ProjeCRM-Mobil", "1")
            .send()
            .await;
        let Ok(yanit) = yanit else { continue };
        if !yanit.status().is_success() {
            continue; // 401: uç henüz çerezli kimliğe açık değil ya da oturum düşmüş
        }
        let Ok(govde) = yanit.json::<BildirimYaniti>().await else { continue };
        for b in govde.bildirimler {
            let yeni = gorulen.insert(b.id.clone());
            if yeni && !ilk {
                let _ = app.notification().builder().title("Proje CRM").body(&b.mesaj).show();
            }
        }
        ilk = false;
    }
}

// ---------------------------------------------------------------- uygulama

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_notification::init())
        .invoke_handler(tauri::generate_handler![panel_oku, panel_ayarla])
        .setup(|app| {
            let handle = app.handle().clone();

            // Ana pencere kodda kurulur: gezinme ve indirme kancaları yalnız builder'da bağlanabiliyor.
            let nav_handle = handle.clone();
            let dl_handle = handle.clone();
            // macOS'ta Finished olayında yol gelmez; istek anındaki hedefi URL'ye göre hatırlarız.
            let indirilenler: Arc<Mutex<HashMap<String, PathBuf>>> = Arc::new(Mutex::new(HashMap::new()));
            WebviewWindowBuilder::new(app, PENCERE, WebviewUrl::App("index.html".into()))
                .title("Proje CRM")
                .inner_size(1280.0, 820.0)
                .min_inner_size(960.0, 640.0)
                .center()
                .on_navigation(move |url| gezinmeye_izin(&nav_handle, url))
                .on_download(move |_webview, olay| {
                    match olay {
                        DownloadEvent::Requested { url, destination } => {
                            let hedef = indirme_hedefi(&dl_handle, &url, destination);
                            indirilenler.lock().unwrap().insert(url.to_string(), hedef.clone());
                            *destination = hedef;
                        }
                        DownloadEvent::Finished { url, path, success } => {
                            let hatirlanan = indirilenler.lock().unwrap().remove(&url.to_string());
                            if success {
                                if let Some(p) = path.or(hatirlanan) {
                                    let ad = p.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                                    let _ = dl_handle.notification().builder().title("İndirildi").body(&ad).show();
                                    let _ = dl_handle.opener().reveal_item_in_dir(&p);
                                }
                            }
                        }
                        _ => {}
                    }
                    true
                })
                .build()?;

            // Kayıtlı panel varsa doğrudan oraya.
            if let Some(origin) = panel_oku_store(&handle) {
                let _ = panele_git(&handle, &origin);
            }

            // Menü: sistem menüsü + Panel alt menüsü (kopyala/yapıştır kısayolları için gerekli).
            let menu = Menu::default(&handle)?;
            let degistir = MenuItem::with_id(&handle, "panel-degistir", "Panel adresini değiştir…", true, Some("CmdOrCtrl+Shift+P"))?;
            let yenile = MenuItem::with_id(&handle, "yenile", "Sayfayı yenile", true, Some("CmdOrCtrl+R"))?;
            let panel_menu = Submenu::with_items(&handle, "Panel", true, &[&yenile, &degistir])?;
            menu.append(&panel_menu)?;
            app.set_menu(menu)?;
            app.on_menu_event(|app, olay| match olay.id().as_ref() {
                "panel-degistir" => {
                    let _ = yerel_ekrana_git(app);
                }
                "yenile" => {
                    if let Some(p) = app.get_webview_window(PENCERE) {
                        let _ = p.eval("location.reload()");
                    }
                }
                _ => {}
            });

            tauri::async_runtime::spawn(bildirim_yokla(handle));
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("Proje CRM başlatılamadı");
}

#[cfg(test)]
mod test {
    use super::panel_origin;

    #[test]
    fn panel_origin_kurallari() {
        assert_eq!(panel_origin("demo.projecrm.com").as_deref(), Some("https://demo.projecrm.com"));
        assert_eq!(panel_origin("  https://x.projecrm.com/ ").as_deref(), Some("https://x.projecrm.com"));
        assert_eq!(panel_origin("localhost:3005").as_deref(), Some("http://localhost:3005"));
        assert_eq!(panel_origin("192.168.1.10:3000").as_deref(), Some("http://192.168.1.10:3000"));
        assert_eq!(panel_origin(""), None);
        assert_eq!(panel_origin("demo.projecrm.com/giris"), None);
        assert_eq!(panel_origin("ne bu"), None);
    }
}
