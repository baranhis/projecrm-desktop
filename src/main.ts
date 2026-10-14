import { invoke } from "@tauri-apps/api/core";

/**
 * Panel adresi ekranı (mobil kabuktaki panel-adresi ekranının masaüstü karşılığı).
 * Tek iş: adresi Rust tarafına verir; doğrulama, saklama ve panele geçiş orada.
 * Kayıtlı adres varsa Rust açılışta zaten panele gitmiştir; bu ekran yalnız ilk
 * kurulumda ve "Panel adresini değiştir" menüsünden görünür.
 */
const form = document.getElementById("form") as HTMLFormElement;
const adres = document.getElementById("adres") as HTMLInputElement;
const hata = document.getElementById("hata") as HTMLParagraphElement;
const devam = document.getElementById("devam") as HTMLButtonElement;

invoke<string | null>("panel_oku")
  .then((o) => {
    if (o) {
      adres.value = o.replace(/^https?:\/\//, "");
      devam.textContent = "Kaydet ve panele dön";
    }
    adres.focus();
  })
  .catch(() => adres.focus());

form.addEventListener("submit", async (e) => {
  e.preventDefault();
  hata.hidden = true;
  devam.disabled = true;
  try {
    await invoke<string>("panel_ayarla", { adres: adres.value });
  } catch (err) {
    hata.textContent = typeof err === "string" ? err : "Panel adresi kaydedilemedi.";
    hata.hidden = false;
    devam.disabled = false;
  }
});
