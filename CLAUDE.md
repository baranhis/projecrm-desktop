# Proje CRM masaüstü (Tauri 2)

Mobil kabukla (projecrm-mobil) aynı felsefe: uygulama = native kabuk + panelin kendisi. Tasarım panelde; burada yalnız
panel adresi ekranı, panel dışı bağlantı yönlendirme, indirme, masaüstü bildirimi ve menü var. Yeni native özellik eklemeden
önce "panel bunu kendisi yapabilir mi?" diye sor.

- Rust: src-tauri/src/lib.rs (tek dosya; büyürse modüllere böl). Değişken/fonksiyon adları Türkçe, yorumlar Türkçe.
- Yerel ekran: index.html + src/main.ts + src/styles.css (yalnız panel adresi).
- Derleme kontrolü: `cargo check` ve `cargo test --lib` (src-tauri içinde). Paket: `npm run tauri build`.
- Windows paketi bu Mac'te üretilemez; `v*` etiketi GitHub Actions'ı tetikler (.github/workflows/release.yml).
- Bildirim yoklayıcı, panelin /api/mobil/v1/bildirimler ucunu çerez + X-ProjeCRM-Mobil başlığıyla çağırır; uç bu
  kimliğe açılana kadar 401 alır ve sessizce geçer.
