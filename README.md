# Proje CRM — masaüstü uygulaması

Mobil kabuğun (projecrm-mobil) masaüstü karşılığı: uygulama = native kabuk (Tauri 2) + panelin kendisi.
Native olan yalnız panel adresi ekranı, panel dışı bağlantıların sisteme yönlendirilmesi, indirmeler,
masaüstü bildirimi ve uygulama menüsü. Görünen her şey web panelidir.

## Geliştirme

```bash
npm install
npm run tauri dev
```

## Paket (yerel)

```bash
npm run tauri build
```

macOS → `src-tauri/target/release/bundle/dmg/`, Windows → `…/bundle/nsis/` (Windows paketi GitHub Actions'ta üretilir).
